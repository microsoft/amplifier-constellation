//! Managed source entries, including real directory pointers on each platform.
//!
//! The serving driver supplies protected, fixed enrollment and holds its named
//! installation lock for mutations. All writers must honor that lock: identity
//! comparisons are before-dispatch checks, not a filesystem compare-and-swap.
//! Domain owners retain source fingerprints, journals, stopped-service checks,
//! and the decision to continue or roll back. An exchange is TWO native renames,
//! with an observable gap. It is not an atomic pair or an app+venv transaction.
//! No operation here deletes an entry, recursively copies files, or retries.

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub current: PathBuf,
    pub roots: BTreeMap<String, PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EntryRef {
    pub root: String,
    pub path: PathBuf,
}

#[derive(Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum MutationOutcome {
    Applied { state: Value },
    Refused { reason: String },
    Unknown { phase: &'static str, error: String },
}

impl MutationOutcome {
    fn refused(reason: &str) -> Self {
        Self::Refused {
            reason: reason.to_owned(),
        }
    }
    fn unknown(phase: &'static str, error: anyhow::Error) -> Self {
        Self::Unknown {
            phase,
            error: format!("{error:#}"),
        }
    }
}

/// None selects the fixed current entry. Observations describe filesystem object
/// identity, never source-content integrity. A missing entry is JSON null.
pub fn inspect(profile: &Profile, entry: Option<&EntryRef>) -> Result<Value> {
    let profile = resolved_profile(profile)?;
    let path = match entry {
        Some(entry) => resolve(&profile, entry)?,
        None => profile.current.clone(),
    };
    inspect_path(&profile, &path)
}

/// Create a symlink (Unix) or directory junction (Windows), never replacing an
/// existing entry. The target must be an observed ordinary directory in a root.
/// On a partial Windows create failure, the created entry is retained for review.
pub fn prepare_pointer(
    profile: &Profile,
    prepared: &EntryRef,
    target: &EntryRef,
    expected_target: &Value,
) -> Result<MutationOutcome> {
    let profile = resolved_profile(profile)?;
    let prepared = resolve(&profile, prepared)?;
    let target = resolve(&profile, target)?;
    separated(&[&profile.current, &prepared, &target])?;
    let actual_target = inspect_path(&profile, &target)?;
    if actual_target.get("kind").and_then(Value::as_str) != Some("directory") {
        return Ok(MutationOutcome::refused(
            "Pointer target is not an ordinary staged directory",
        ));
    }
    if &actual_target != expected_target {
        return Ok(MutationOutcome::refused("Prepared target identity changed"));
    }
    if !inspect_path(&profile, &prepared)?.is_null() {
        return Ok(MutationOutcome::refused("Prepared entry already exists"));
    }
    same_volume(&[
        profile.current.parent().unwrap(),
        prepared.parent().unwrap(),
        &target,
    ])?;
    if let Err(error) = create_pointer(&prepared, &target) {
        return Ok(MutationOutcome::unknown("prepare_pointer", error));
    }
    if let Err(error) = sync_parent(&prepared) {
        return Ok(MutationOutcome::unknown("prepare_pointer_sync", error));
    }
    match inspect_path(&profile, &prepared) {
        Ok(state) if state.get("target_identity") == actual_target.get("identity") => {
            Ok(MutationOutcome::Applied { state })
        }
        Ok(_) => Ok(MutationOutcome::unknown(
            "prepare_pointer_verify",
            anyhow::anyhow!("Prepared pointer target changed"),
        )),
        Err(error) => Ok(MutationOutcome::unknown("prepare_pointer_verify", error)),
    }
}

/// Retain current, then install incoming. The retained destination must be
/// absent, even if an existing entry appears identical. A rollback is another
/// explicitly journaled exchange using the retained original as incoming and a
/// fresh retained destination. Fresh installation permits expected_current=null.
pub fn exchange(
    profile: &Profile,
    incoming: &EntryRef,
    retained: &EntryRef,
    expected_current: &Value,
    expected_incoming: &Value,
) -> Result<MutationOutcome> {
    let profile = resolved_profile(profile)?;
    let incoming = resolve(&profile, incoming)?;
    let retained = resolve(&profile, retained)?;
    separated(&[&profile.current, &incoming, &retained])?;
    let current = inspect_path(&profile, &profile.current)?;
    let next = inspect_path(&profile, &incoming)?;
    let backup = inspect_path(&profile, &retained)?;
    if &current != expected_current || &next != expected_incoming || next.is_null() {
        return Ok(MutationOutcome::refused(
            "Current or prepared entry identity changed",
        ));
    }
    if !backup.is_null() {
        return Ok(MutationOutcome::refused(
            "Retained destination already exists",
        ));
    }
    // Moving a pointer must not also move an ancestor of its own target.
    let mut topology = vec![profile.current.clone(), incoming.clone(), retained.clone()];
    for state in [&current, &next] {
        if let Some(target) = state.get("target").and_then(Value::as_str) {
            let target = PathBuf::from(target);
            for entry in [&profile.current, &incoming, &retained] {
                ensure!(
                    !overlaps(entry, &target),
                    "Source entry overlaps a pointer target"
                );
            }
            topology.push(target);
        }
    }
    let mut volumes = vec![
        profile.current.parent().unwrap(),
        incoming.parent().unwrap(),
        retained.parent().unwrap(),
    ];
    if current.get("kind").and_then(Value::as_str) == Some("directory") {
        volumes.push(&profile.current);
    }
    if next.get("kind").and_then(Value::as_str) == Some("directory") {
        volumes.push(&incoming);
    }
    volumes.extend(topology.iter().skip(3).map(PathBuf::as_path));
    same_volume(&volumes)?;
    // Each rename is no-replace, so even an unexpected destination is preserved.
    if !current.is_null()
        && let Err(error) = rename_entry(&profile.current, &retained)
    {
        return Ok(MutationOutcome::unknown("retain_current", error));
    }
    if let Err(error) = rename_entry(&incoming, &profile.current) {
        return Ok(MutationOutcome::unknown("install_prepared", error));
    }
    let observed = (|| -> Result<Value> {
        let after = inspect_path(&profile, &profile.current)?;
        let saved = inspect_path(&profile, &retained)?;
        let remaining = inspect_path(&profile, &incoming)?;
        ensure!(
            after == next && saved == current && remaining.is_null(),
            "Source entries changed during exchange"
        );
        Ok(json!({"current":after,"incoming":remaining,"retained":saved}))
    })();
    Ok(match observed {
        Ok(state) => MutationOutcome::Applied { state },
        Err(error) => MutationOutcome::unknown("exchange_verify", error),
    })
}

fn resolved_profile(profile: &Profile) -> Result<Profile> {
    let mut roots = BTreeMap::new();
    for (name, root) in &profile.roots {
        ensure!(!name.is_empty(), "Empty approved root name");
        ordinary_chain(root)?;
        ensure!(
            fs::symlink_metadata(root)?.is_dir(),
            "Approved root is not a directory"
        );
        roots.insert(name.clone(), fs::canonicalize(root)?);
    }
    ensure!(!roots.is_empty(), "No approved source roots");
    let current = normalized_leaf(&profile.current)?;
    Ok(Profile { current, roots })
}

fn resolve(profile: &Profile, entry: &EntryRef) -> Result<PathBuf> {
    let root = profile
        .roots
        .get(&entry.root)
        .context("Unenrolled source root")?;
    ensure!(
        !entry.path.as_os_str().is_empty()
            && entry
                .path
                .components()
                .all(|p| matches!(p, Component::Normal(_))),
        "Source entry must be a nonempty relative path without traversal"
    );
    #[cfg(windows)]
    ensure!(
        !entry.path.as_os_str().to_string_lossy().contains(':'),
        "Alternate data streams are not source entries"
    );
    normalized_leaf(&root.join(&entry.path))
}

fn normalized_leaf(path: &Path) -> Result<PathBuf> {
    ensure!(path.is_absolute(), "Source paths must be absolute");
    let parent = path.parent().context("Source entry needs a parent")?;
    let name = path.file_name().context("Source entry needs a name")?;
    ordinary_chain(parent)?;
    Ok(fs::canonicalize(parent)?.join(name))
}

pub(crate) fn ordinary_chain(path: &Path) -> Result<()> {
    ensure!(
        path.is_absolute()
            && path
                .components()
                .all(|p| !matches!(p, Component::ParentDir | Component::CurDir)),
        "Source path contains traversal"
    );
    for ancestor in path.ancestors() {
        let metadata = fs::symlink_metadata(ancestor)?;
        ensure!(
            metadata.is_dir() && !is_pointer(&metadata),
            "Source ancestor is not an ordinary directory"
        );
    }
    Ok(())
}

fn inspect_path(profile: &Profile, path: &Path) -> Result<Value> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Value::Null),
        Err(error) => return Err(error.into()),
    };
    let identity = native_identity(path, &metadata)?;
    if !is_pointer(&metadata) {
        ensure!(
            metadata.is_dir(),
            "Source entry is not a directory or managed pointer"
        );
        return Ok(json!({"kind":"directory","identity":identity}));
    }
    validate_pointer_type(path, &metadata)?;
    let raw = fs::read_link(path)?;
    ensure!(
        raw.is_absolute(),
        "Managed directory pointers must have absolute targets"
    );
    ordinary_chain(&raw)?;
    let target = fs::canonicalize(raw)?;
    ensure!(
        profile.roots.values().any(|root| beneath(root, &target)),
        "Pointer target is outside approved source roots"
    );
    let target_metadata = fs::symlink_metadata(&target)?;
    ensure!(
        target_metadata.is_dir() && !is_pointer(&target_metadata),
        "Pointer target is not an ordinary directory"
    );
    Ok(json!({"kind":"pointer","identity":identity,"target":target,
        "target_identity":native_identity(&target,&target_metadata)?}))
}

fn separated(paths: &[&Path]) -> Result<()> {
    for (i, a) in paths.iter().enumerate() {
        for b in paths.iter().skip(i + 1) {
            ensure!(
                !overlaps(a, b),
                "Current, prepared and retained source entries overlap"
            );
        }
    }
    Ok(())
}
fn overlaps(a: &Path, b: &Path) -> bool {
    beneath(a, b) || beneath(b, a)
}
fn beneath(parent: &Path, child: &Path) -> bool {
    child.ancestors().any(|p| same_path(parent, p))
}
#[cfg(not(windows))]
fn same_path(a: &Path, b: &Path) -> bool {
    a == b
}
#[cfg(windows)]
fn same_path(a: &Path, b: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Globalization::{CSTR_EQUAL, CompareStringOrdinal};
    let a: Vec<u16> = a.as_os_str().encode_wide().collect();
    let b: Vec<u16> = b.as_os_str().encode_wide().collect();
    unsafe {
        CompareStringOrdinal(a.as_ptr(), a.len() as i32, b.as_ptr(), b.len() as i32, 1)
            == CSTR_EQUAL
    }
}

fn same_volume(paths: &[&Path]) -> Result<()> {
    let mut expected = None;
    for path in paths {
        let metadata = fs::symlink_metadata(path)?;
        let identity = native_identity(path, &metadata)?;
        let volume = identity
            .get("volume")
            .context("Missing native volume identity")?;
        if let Some(expected) = &expected {
            ensure!(
                expected == volume,
                "Source exchange requires the same filesystem"
            );
        } else {
            expected = Some(volume.clone());
        }
    }
    Ok(())
}

fn rename_entry(from: &Path, to: &Path) -> Result<()> {
    crate::storage::publish_directory_native(from, to)?;
    sync_parent(from)?;
    if from.parent() != to.parent() {
        sync_parent(to)?;
    }
    Ok(())
}
fn sync_parent(path: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::File::open(path.parent().context("Missing source parent")?)?.sync_all()?;
    #[cfg(windows)]
    let _ = path; // Native rename uses MOVEFILE_WRITE_THROUGH.
    Ok(())
}

#[cfg(unix)]
pub(crate) fn is_pointer(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}
#[cfg(windows)]
pub(crate) fn is_pointer(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes()
        & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
        != 0
}
#[cfg(unix)]
pub(crate) fn native_identity(_: &Path, metadata: &fs::Metadata) -> Result<Value> {
    use std::os::unix::fs::MetadataExt;
    Ok(
        json!({"platform":"unix","volume":metadata.dev().to_string(),"object":metadata.ino().to_string()}),
    )
}
#[cfg(unix)]
fn validate_pointer_type(_: &Path, metadata: &fs::Metadata) -> Result<()> {
    ensure!(
        metadata.file_type().is_symlink(),
        "Unknown source pointer type"
    );
    Ok(())
}
#[cfg(unix)]
fn create_pointer(prepared: &Path, target: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, prepared)?;
    Ok(())
}

#[cfg(windows)]
const JUNCTION_TAG: u32 = 0xa000_0003; // IO_REPARSE_TAG_MOUNT_POINT

#[cfg(windows)]
fn wide(path: &Path) -> Result<Vec<u16>> {
    crate::storage::wide_path(path)
}

#[cfg(windows)]
fn open_entry(path: &Path, access: u32) -> Result<fs::File> {
    use std::os::windows::io::FromRawHandle;
    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        Storage::FileSystem::{
            CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        },
    };
    let name = wide(path)?;
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    ensure!(
        handle != INVALID_HANDLE_VALUE,
        "Cannot open source entry: {}",
        std::io::Error::last_os_error()
    );
    Ok(unsafe { fs::File::from_raw_handle(handle) })
}

#[cfg(windows)]
pub(crate) fn native_identity(path: &Path, metadata: &fs::Metadata) -> Result<Value> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle, GetVolumeNameForVolumeMountPointW,
        GetVolumePathNameW,
    };
    let file = open_entry(path, 0)?;
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    ensure!(
        unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } != 0,
        "Cannot inspect source identity: {}",
        std::io::Error::last_os_error()
    );
    let volume_path = if is_pointer(metadata) {
        path.parent().context("Pointer parent missing")?
    } else {
        path
    };
    let name = wide(volume_path)?;
    let mut mount = vec![0u16; 32768];
    ensure!(
        unsafe { GetVolumePathNameW(name.as_ptr(), mount.as_mut_ptr(), mount.len() as u32) } != 0,
        "Cannot locate source volume: {}",
        std::io::Error::last_os_error()
    );
    let mut volume = vec![0u16; 64];
    ensure!(
        unsafe {
            GetVolumeNameForVolumeMountPointW(
                mount.as_ptr(),
                volume.as_mut_ptr(),
                volume.len() as u32,
            )
        } != 0,
        "Cannot identify source volume: {}",
        std::io::Error::last_os_error()
    );
    let end = volume
        .iter()
        .position(|c| *c == 0)
        .context("Invalid source volume identity")?;
    let volume = String::from_utf16(&volume[..end])?;
    let object = ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64;
    Ok(json!({"platform":"windows","volume":volume,"object":object.to_string()}))
}

#[cfg(windows)]
fn validate_pointer_type(path: &Path, _: &fs::Metadata) -> Result<()> {
    ensure!(
        reparse_tag(path)? == JUNCTION_TAG,
        "Only directory junctions are managed source pointers"
    );
    Ok(())
}

#[cfg(windows)]
pub(crate) fn reparse_tag(path: &Path) -> Result<u32> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_TAG_INFO, FileAttributeTagInfo, GetFileInformationByHandleEx,
    };
    let file = open_entry(path, 0)?;
    let mut info: FILE_ATTRIBUTE_TAG_INFO = unsafe { std::mem::zeroed() };
    ensure!(
        unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle(),
                FileAttributeTagInfo,
                (&mut info as *mut FILE_ATTRIBUTE_TAG_INFO).cast(),
                std::mem::size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
            )
        } != 0,
        "Cannot inspect source reparse type: {}",
        std::io::Error::last_os_error()
    );
    Ok(info.ReparseTag)
}

#[cfg(windows)]
fn create_pointer(prepared: &Path, target: &Path) -> Result<()> {
    use std::os::windows::{ffi::OsStrExt, io::AsRawHandle};
    use windows_sys::Win32::{
        Foundation::GENERIC_WRITE,
        System::{IO::DeviceIoControl, Ioctl::FSCTL_SET_REPARSE_POINT},
    };
    let mut print: Vec<u16> = target.as_os_str().encode_wide().collect();
    if print.starts_with(&[92, 92, 63, 92]) {
        print.drain(..4);
    }
    ensure!(
        print.len() >= 3
            && print[0] <= 127
            && (print[0] as u8).is_ascii_alphabetic()
            && print[1] == 58
            && print[2] == 92,
        "Directory junctions require a local absolute drive target"
    );
    let mut substitute: Vec<u16> = vec![92, 63, 63, 92];
    substitute.extend_from_slice(&print);
    let substitute_bytes = substitute.len() * 2;
    let print_bytes = print.len() * 2;
    let data_bytes = 8 + substitute_bytes + 2 + print_bytes + 2;
    ensure!(
        data_bytes + 8 <= 16 * 1024,
        "Directory junction target is too long"
    );
    let mut buffer = Vec::with_capacity(data_bytes + 8);
    buffer.extend_from_slice(&JUNCTION_TAG.to_le_bytes());
    buffer.extend_from_slice(&(data_bytes as u16).to_le_bytes());
    buffer.extend_from_slice(&0u16.to_le_bytes());
    for field in [
        0u16,
        substitute_bytes as u16,
        (substitute_bytes + 2) as u16,
        print_bytes as u16,
    ] {
        buffer.extend_from_slice(&field.to_le_bytes());
    }
    for unit in substitute.into_iter().chain([0]).chain(print).chain([0]) {
        buffer.extend_from_slice(&unit.to_le_bytes());
    }
    // Protected ACL is supplied at creation, not inherited then repaired. A
    // failure after this point preserves the entry; the domain inspects it.
    crate::storage::create_private_directory(prepared)?;
    let file = open_entry(prepared, GENERIC_WRITE)?;
    let mut returned = 0;
    ensure!(
        unsafe {
            DeviceIoControl(
                file.as_raw_handle(),
                FSCTL_SET_REPARSE_POINT,
                buffer.as_ptr().cast(),
                buffer.len() as u32,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            )
        } != 0,
        "Cannot prepare directory junction: {}",
        std::io::Error::last_os_error()
    );
    Ok(())
}
