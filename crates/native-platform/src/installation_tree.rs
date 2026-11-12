//! Removal of one explicitly selected regenerable directory below a fixed root.
//!
//! The caller supplies protected enrollment and holds its installation lock for
//! both domain reference checks and removal. Snapshots are object identities,
//! not content fingerprints or filesystem CAS against hostile same-owner writes.
//! Domain ownership, references, pruning intent and recovery stay with the caller.
//! The standard library owns deletion; this module only validates its boundary.

pub use crate::installation_exchange::MutationOutcome;
use crate::installation_exchange::{is_pointer, native_identity, ordinary_chain};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

/// Cheap, nonrecursive observation. A missing child returns null. Existing
/// children must be direct ordinary directories; pointers are never tree roots.
pub fn inspect(root: &Path, entry: &str) -> Result<Value> {
    let path = child_path(root, entry)?;
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Value::Null),
        Err(error) => return Err(error.into()),
    };
    ensure!(
        metadata.is_dir() && !is_pointer(&metadata),
        "Tree child is not an ordinary directory"
    );
    let root = path.parent().context("Missing tree root")?;
    Ok(json!({
        "schema":1, "root":root, "root_identity":native_identity(root, &fs::symlink_metadata(root)?)?,
        "entry":entry, "path":path, "identity":native_identity(&path, &metadata)?
    }))
}

/// Identity-check and preflight the whole tree, then remove it without following
/// links. Once standard-library traversal starts, every error is Unknown. There
/// is no retry or detached user-space traversal. An interrupted native operation
/// may still have pending kernel effects. Success means observed absence only.
/// `protected_parents` must contain the config parent and every enrolled lock's
/// parent, derived by the driver from protected enrollment. Their native object
/// identities are excluded even when a bind alias hides lexical containment.
pub fn remove(
    root: &Path,
    entry: &str,
    expected: &Value,
    protected_parents: &[PathBuf],
) -> Result<MutationOutcome> {
    remove_with(root, entry, expected, protected_parents, |path| {
        fs::remove_dir_all(path)
    })
}

fn remove_with(
    root: &Path,
    entry: &str,
    expected: &Value,
    protected_parents: &[PathBuf],
    delete: impl FnOnce(&Path) -> io::Result<()>,
) -> Result<MutationOutcome> {
    let before = inspect(root, entry)?;
    if before.is_null() || &before != expected {
        return Ok(refused("Tree root or child identity changed"));
    }
    let path = child_path(root, entry)?;
    if let Err(error) = preflight(&path, &before["root_identity"]["volume"], protected_parents) {
        return Ok(refused(&format!("Tree preflight refused: {error:#}")));
    }
    // Recheck after potentially lengthy enumeration. Cooperating writers still
    // need the same lock; this does not make the following path call atomic.
    if inspect(root, entry)? != before {
        return Ok(refused("Tree identity changed during preflight"));
    }
    if let Err(error) = delete(&path) {
        return Ok(unknown("tree_remove", error.into()));
    }
    let observed = (|| -> Result<Value> {
        let child = child_path(root, entry)?;
        let parent = child.parent().context("Missing tree root")?;
        ensure!(
            native_identity(parent, &fs::symlink_metadata(parent)?)? == before["root_identity"],
            "Tree root changed during removal"
        );
        inspect(root, entry)
    })();
    match observed {
        Ok(state) if state.is_null() => Ok(MutationOutcome::Applied {
            state: json!({"absent":true}),
        }),
        Ok(_) => Ok(unknown(
            "tree_remove_verify",
            anyhow::anyhow!("Tree entry exists after removal"),
        )),
        Err(error) => Ok(unknown("tree_remove_verify", error)),
    }
}

fn refused(reason: &str) -> MutationOutcome {
    MutationOutcome::Refused {
        reason: reason.to_owned(),
    }
}
fn unknown(phase: &'static str, error: anyhow::Error) -> MutationOutcome {
    MutationOutcome::Unknown {
        phase,
        error: format!("{error:#}"),
    }
}

/// Validate the single selected child name without a filesystem observation.
pub fn validate_entry(entry: &str) -> Result<()> {
    ensure!(
        !entry.is_empty()
            && entry != "."
            && entry != ".."
            && !entry.contains(['/', '\\', ':', '\0'])
            && !entry.ends_with(['.', ' '])
            && Path::new(entry).components().count() == 1
            && matches!(
                Path::new(entry).components().next(),
                Some(Component::Normal(_))
            ),
        "Tree entry must be exactly one normal child name"
    );
    Ok(())
}

fn child_path(root: &Path, entry: &str) -> Result<PathBuf> {
    validate_entry(entry)?;
    ordinary_chain(root)?;
    Ok(fs::canonicalize(root)?.join(entry))
}

fn preflight(path: &Path, volume: &Value, protected_parents: &[PathBuf]) -> Result<()> {
    // Device comparisons alone miss bind mounts on the same filesystem. Snapshot
    // the native mount table before walking; no mount or privilege is changed.
    refuse_mounts(path)?;
    // A preexisting bind alias at the capability root can make a control parent
    // appear under a different lexical path without crossing a descendant mount.
    // The serving driver supplies the config parent and every enrolled lock's
    // parent, all from protected configuration, never from the request.
    let protected: Vec<Value> = protected_parents
        .iter()
        .map(|parent| {
            ordinary_chain(parent)?;
            native_identity(parent, &fs::symlink_metadata(parent)?)
        })
        .collect::<Result<_>>()?;
    let mut pending = vec![path.to_owned()];
    while let Some(directory) = pending.pop() {
        let metadata = fs::symlink_metadata(&directory)?;
        ensure!(
            metadata.is_dir() && !is_pointer(&metadata),
            "Tree directory changed during preflight"
        );
        let identity = native_identity(&directory, &metadata)?;
        ensure!(
            identity["volume"] == *volume,
            "Tree crosses a filesystem boundary"
        );
        ensure!(
            !protected.contains(&identity),
            "Tree contains a protected control directory"
        );
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let child = entry.path();
            let metadata = fs::symlink_metadata(&child)?;
            if is_pointer(&metadata) {
                validate_leaf_pointer(&child, &metadata)?;
            } else if metadata.is_dir() {
                pending.push(child);
            } else {
                ensure!(
                    metadata.is_file(),
                    "Tree contains an unsupported filesystem entry"
                );
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    ensure!(
                        metadata.dev().to_string()
                            == volume.as_str().context("Missing tree volume")?,
                        "Tree file crosses a filesystem boundary"
                    );
                }
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn validate_leaf_pointer(_: &Path, metadata: &fs::Metadata) -> Result<()> {
    ensure!(
        metadata.file_type().is_symlink(),
        "Unsupported tree pointer"
    );
    Ok(())
}

#[cfg(windows)]
fn validate_leaf_pointer(path: &Path, _: &fs::Metadata) -> Result<()> {
    use std::path::Prefix;
    match crate::installation_exchange::reparse_tag(path)? {
        0xa000_000c => Ok(()), // IO_REPARSE_TAG_SYMLINK: delete the link only.
        0xa000_0003 => {
            // A volume mount uses the same tag as a directory junction. Only
            // ordinary local-drive junction targets are supported here; volume
            // GUID mounts and other reparse forms are preserved and refused.
            let target = fs::read_link(path)?;
            ensure!(
                matches!(target.components().next(), Some(Component::Prefix(prefix))
                if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))),
                "Tree contains a volume mount or unsupported junction"
            );
            Ok(())
        }
        _ => anyhow::bail!("Tree contains an unsupported reparse point"),
    }
}

#[cfg(target_os = "linux")]
fn refuse_mounts(path: &Path) -> Result<()> {
    // Field 5 is the mount point, including same-device bind mounts. Kernel
    // octal escaping preserves spaces, tabs, newlines and backslashes.
    // https://docs.kernel.org/filesystems/proc.html#proc-pid-mountinfo
    let table = fs::read("/proc/self/mountinfo").context("Cannot observe mount boundaries")?;
    for mount in linux_mount_points(&table)? {
        ensure!(!mount.starts_with(path), "Tree contains a mount point");
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn linux_mount_points(table: &[u8]) -> Result<Vec<PathBuf>> {
    ensure!(table.ends_with(b"\n"), "Truncated mount information");
    let mut mounts = Vec::new();
    for line in table[..table.len() - 1].split(|b| *b == b'\n') {
        ensure!(
            !line.iter().any(|b| *b < b' ' || *b == 127),
            "Raw control byte in mount information"
        );
        let fields: Vec<&[u8]> = line.split(|b| *b == b' ').collect();
        ensure!(
            fields.len() >= 10 && fields.iter().all(|field| !field.is_empty()),
            "Truncated mount row"
        );
        mount_number(fields[0])?;
        mount_number(fields[1])?;
        let device: Vec<&[u8]> = fields[2].split(|b| *b == b':').collect();
        ensure!(device.len() == 2, "Malformed mount device");
        mount_number(device[0])?;
        mount_number(device[1])?;
        let separator = fields
            .iter()
            .skip(6)
            .position(|field| *field == b"-")
            .map(|index| index + 6)
            .context("Missing mount separator")?;
        ensure!(
            separator + 4 == fields.len(),
            "Malformed mount trailing fields"
        );
        // Unknown optional fields are deliberately accepted, as required by the
        // kernel format; mandatory IDs/device/paths and field boundaries are not.
        validate_mount_root(fields[3], fields[separator + 1])?;
        mounts.push(decode_mount_path(fields[4])?);
    }
    ensure!(!mounts.is_empty(), "Mount information is empty");
    Ok(mounts)
}

#[cfg(target_os = "linux")]
fn validate_mount_root(field: &[u8], filesystem: &[u8]) -> Result<()> {
    if filesystem == b"nsfs" && !field.starts_with(b"/") {
        // Bind-mounted namespace handles have a kernel-generated dentry name,
        // not a filesystem root path: ns_dname emits "%s:[%llu]". Their actual
        // mountpoint is still validated as an absolute normal path below.
        // https://github.com/torvalds/linux/blob/master/fs/nsfs.c
        let separator = field
            .windows(2)
            .position(|pair| pair == b":[")
            .context("Malformed namespace mount root")?;
        let name = &field[..separator];
        ensure!(
            !name.is_empty() && name.iter().all(|b| b.is_ascii_lowercase() || *b == b'_'),
            "Malformed namespace type"
        );
        let number = field[separator + 2..]
            .strip_suffix(b"]")
            .context("Malformed namespace mount identity")?;
        mount_number(number)?;
        return Ok(());
    }
    decode_mount_path(field)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn mount_number(field: &[u8]) -> Result<()> {
    ensure!(
        !field.is_empty() && field.iter().all(u8::is_ascii_digit),
        "Malformed mount number"
    );
    std::str::from_utf8(field)?
        .parse::<u64>()
        .context("Mount number overflow")?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn decode_mount_path(field: &[u8]) -> Result<PathBuf> {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let mut decoded = Vec::with_capacity(field.len());
    let mut index = 0;
    while index < field.len() {
        if field[index] == b'\\' {
            let escape = field
                .get(index + 1..index + 4)
                .context("Truncated mount escape")?;
            let byte = match escape {
                b"040" => b' ',
                b"011" => b'\t',
                b"012" => b'\n',
                b"134" => b'\\',
                _ => anyhow::bail!("Invalid mount escape"),
            };
            decoded.push(byte);
            index += 4;
        } else {
            decoded.push(field[index]);
            index += 1;
        }
    }
    ensure!(
        decoded.starts_with(b"/")
            && (decoded == b"/"
                || decoded[1..]
                    .split(|b| *b == b'/')
                    .all(|part| !part.is_empty() && part != b"." && part != b"..")),
        "Mount path is not absolute and normal"
    );
    Ok(PathBuf::from(OsString::from_vec(decoded)))
}

#[cfg(target_os = "macos")]
fn refuse_mounts(path: &Path) -> Result<()> {
    use std::{
        ffi::{CStr, OsStr},
        os::unix::ffi::OsStrExt,
    };
    // Caller-owned storage avoids getmntinfo's shared static buffer. Refuse if
    // mount-table growth fills the buffer instead of accepting a partial view.
    let count = unsafe { libc::getfsstat(std::ptr::null_mut(), 0, libc::MNT_NOWAIT) };
    ensure!(
        count > 0,
        "Cannot count mount boundaries: {}",
        io::Error::last_os_error()
    );
    let capacity = usize::try_from(count)?
        .checked_add(8)
        .context("Too many mounts")?;
    let mut mounts: Vec<libc::statfs> = (0..capacity)
        .map(|_| unsafe { std::mem::zeroed() })
        .collect();
    let bytes = i32::try_from(
        capacity
            .checked_mul(std::mem::size_of::<libc::statfs>())
            .context("Mount table too large")?,
    )?;
    let count = unsafe { libc::getfsstat(mounts.as_mut_ptr(), bytes, libc::MNT_NOWAIT) };
    ensure!(
        count > 0 && (count as usize) < capacity,
        "Cannot observe complete mount boundaries"
    );
    for mount in mounts.iter().take(count as usize) {
        let name = unsafe { CStr::from_ptr(mount.f_mntonname.as_ptr()) };
        ensure!(
            !Path::new(OsStr::from_bytes(name.to_bytes())).starts_with(path),
            "Tree contains a mount point"
        );
    }
    Ok(())
}

#[cfg(windows)]
fn refuse_mounts(_: &Path) -> Result<()> {
    // Mounted folders are reparse points, checked without traversal below.
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn refuse_mounts(_: &Path) -> Result<()> {
    anyhow::bail!("Native mount boundary observation is unavailable on this platform")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_after_partial_deletion_is_unknown_and_does_not_retry() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let child = root.join("candidate");
        fs::create_dir(&child).unwrap();
        fs::write(child.join("first"), "first").unwrap();
        fs::write(child.join("retained"), "retained").unwrap();
        let snapshot = inspect(&root, "candidate").unwrap();
        let outcome = remove_with(&root, "candidate", &snapshot, &[], |path| {
            fs::remove_file(path.join("first"))?;
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "fixture failure after first effect",
            ))
        })
        .unwrap();
        assert!(
            matches!(
                &outcome,
                MutationOutcome::Unknown {
                    phase: "tree_remove",
                    ..
                }
            ),
            "Unexpected partial-deletion outcome: {outcome:?}"
        );
        assert!(!child.join("first").exists());
        assert_eq!(
            fs::read_to_string(child.join("retained")).unwrap(),
            "retained"
        );
    }

    #[test]
    fn replaced_root_after_deletion_cannot_confirm_absence() {
        let temporary = tempfile::tempdir().unwrap();
        let home = temporary.path().canonicalize().unwrap();
        let root = home.join("stages");
        fs::create_dir_all(root.join("candidate")).unwrap();
        let before = inspect(&root, "candidate").unwrap();
        let outcome = remove_with(&root, "candidate", &before, &[], |path| {
            fs::remove_dir_all(path)?;
            fs::rename(&root, home.join("old-stages"))?;
            fs::create_dir(&root)
        })
        .unwrap();
        assert!(
            matches!(
                &outcome,
                MutationOutcome::Unknown {
                    phase: "tree_remove_verify",
                    ..
                }
            ),
            "Unexpected root-replacement outcome: {outcome:?}"
        );
        assert!(root.is_dir());
        assert!(home.join("old-stages").is_dir());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn mount_table_preserves_same_device_bind_and_escaped_names() {
        let mounts = linux_mount_points(b"20 1 0:1 / / rw - tmpfs none rw\n21 20 0:1 /outside /root/stage/with\\040space rw - tmpfs none rw\n").unwrap();
        assert!(mounts[1].starts_with("/root/stage"));
        assert_eq!(mounts[1], Path::new("/root/stage/with space"));
        assert!(linux_mount_points(b"20 1 0:1 / /bad\\999 rw - tmpfs none rw").is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn real_namespace_mount_root_is_not_misclassified_as_a_relative_path() {
        let row = b"6881 6861 0:5 net:[4026534758] /run/docker/netns/012f174fd1cb rw master:2726 - nsfs nsfs rw\n";
        assert_eq!(
            linux_mount_points(row).unwrap(),
            [PathBuf::from("/run/docker/netns/012f174fd1cb")]
        );
        let mounted_namespace = b"4280 2511 0:5 mnt:[4026534584] /run/snapd/ns/snapd-desktop-integration.mnt rw - nsfs nsfs rw\n";
        assert_eq!(
            linux_mount_points(mounted_namespace).unwrap(),
            [PathBuf::from("/run/snapd/ns/snapd-desktop-integration.mnt")]
        );
        for invalid in [
            "1 1 0:5 net:[4026534758] /safe rw - tmpfs none rw\n",
            "1 1 0:5 net:[x] /safe rw - nsfs nsfs rw\n",
            "1 1 0:5 net:[42]suffix /safe rw - nsfs nsfs rw\n",
            "1 1 0:5 ../net:[42] /safe rw - nsfs nsfs rw\n",
            "1 1 0:5 net:[42] relative rw - nsfs nsfs rw\n",
            "1 1 0:5 net:[42] /../escape rw - nsfs nsfs rw\n",
        ] {
            assert!(
                linux_mount_points(invalid.as_bytes()).is_err(),
                "Accepted {invalid:?}"
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn malformed_mount_table_refuses_instead_of_accepting_a_partial_view() {
        for row in [
            "1 1 0:1 / /\n",
            "garbage 1 0:1 / / rw - tmpfs none rw\n",
            "1 parent 0:1 / / rw - tmpfs none rw\n",
            "1 1 nope / / rw - tmpfs none rw\n",
            "1 1 0:x / / rw - tmpfs none rw\n",
            "1 1 0:1 / / rw tmpfs none rw\n",
            "1 1 0:1 / / rw - tmpfs none\n",
            "1 1 0:1 / / rw - tmpfs none rw trailing\n",
            "1 1 0:1 / /bad\0path rw - tmpfs none rw\n",
            "1 1 0:1 / /../escape rw - tmpfs none rw\n",
            "1 1 0:1 /./root /safe rw - tmpfs none rw\n",
            "1 1 0:1 relative /safe rw - tmpfs none rw\n",
            "1 1 0:1 / /safe rw - tmpfs none rw",
            "1 1 0:1 / /safe rw - tmpfs none rw\n\n",
            "1 1 0:1 / /safe rw - tmpfs none rw\ntruncated",
        ] {
            assert!(
                linux_mount_points(row.as_bytes()).is_err(),
                "Accepted {row:?}"
            );
        }
    }
}
