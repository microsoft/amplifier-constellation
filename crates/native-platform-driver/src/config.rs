use amplifier_native_platform::{
    installation::Service, installation_exchange::Profile as ExchangeProfile,
    registration::Registration,
};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    #[serde(default)]
    pub roots: BTreeMap<String, PathBuf>,
    #[serde(default)]
    pub locks: BTreeMap<String, PathBuf>,
    #[serde(default)]
    pub services: BTreeMap<String, Service>,
    #[serde(default)]
    pub commands: BTreeMap<String, CommandProfile>,
    #[serde(default)]
    pub service_applications: BTreeMap<String, crate::service_application::Profile>,
    #[serde(default)]
    pub registrations: BTreeMap<String, Registration>,
    #[serde(default)]
    pub exchanges: BTreeMap<String, EnrolledExchange>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnrolledExchange {
    pub profile: ExchangeProfile,
    pub lock: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandProfile {
    pub executable: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub timeout_seconds: u64,
    pub result_root: Option<String>,
}

pub struct Enrollment {
    pub path: PathBuf,
    digest: Vec<u8>,
    pub config: Config,
}

impl Enrollment {
    pub fn load(path: &Path) -> Result<Self> {
        protected_file(path)?;
        let bytes = fs::read(path)?;
        ensure!(bytes.len() <= 1024 * 1024, "Configuration is too large");
        let config: Config = serde_json::from_slice(&bytes)?;
        ensure!(config.version == 1, "Unsupported configuration version");
        for name in config
            .roots
            .keys()
            .chain(config.locks.keys())
            .chain(config.services.keys())
            .chain(config.commands.keys())
            .chain(config.service_applications.keys())
            .chain(config.registrations.keys())
            .chain(config.exchanges.keys())
        {
            ensure!(valid_name(name), "Invalid enrolled name");
        }
        for root in config.roots.values() {
            protected_directory(root)?;
        }
        for lock in config.locks.values() {
            ensure!(lock.is_absolute(), "Lock must be absolute");
            protected_directory(lock.parent().context("Lock needs a parent")?)?;
            path_chain(lock)?;
            if lock.exists() {
                protected_file(lock)?;
            }
        }
        for command in config.commands.values() {
            ensure!(
                command.executable.is_absolute() && command.executable.is_file(),
                "Command executable must be an installed absolute file"
            );
            ensure!(
                command.cwd.is_absolute() && command.cwd.is_dir(),
                "Command working directory must exist"
            );
            ensure!(
                (1..=300).contains(&command.timeout_seconds),
                "Command bound must be 1..300 seconds"
            );
            ensure!(
                command.args.iter().all(|s| !s.contains('\0')),
                "Invalid command argument"
            );
            let placeholders = command
                .args
                .iter()
                .filter(|s| s.as_str() == "{result}")
                .count();
            match &command.result_root {
                Some(root) => ensure!(
                    config.roots.contains_key(root) && placeholders == 1,
                    "Result command needs one placeholder and an enrolled root"
                ),
                None => ensure!(
                    placeholders == 0,
                    "Result placeholder needs an enrolled root"
                ),
            }
        }
        for (name, application) in &config.service_applications {
            ensure!(
                config.services.contains_key(name),
                "Application needs an enrolled service"
            );
            ensure!(
                config.registrations.contains_key(name),
                "Service application needs its enrolled registration profile"
            );
            application.validate()?;
        }
        for name in config.registrations.keys() {
            ensure!(
                config.services.contains_key(name),
                "Registration needs an enrolled service"
            );
        }
        for exchange in config.exchanges.values() {
            ensure!(
                config.locks.contains_key(&exchange.lock),
                "Exchange needs its enrolled critical-section lock"
            );
            protected_directory(
                exchange
                    .profile
                    .current
                    .parent()
                    .context("Current entry needs a parent")?,
            )?;
            for root in exchange.profile.roots.values() {
                protected_directory(root)?;
            }
        }
        Ok(Self {
            path: path.to_owned(),
            digest: Sha256::digest(&bytes).to_vec(),
            config,
        })
    }

    pub fn unchanged(&self) -> Result<()> {
        protected_file(&self.path)?;
        ensure!(
            Sha256::digest(fs::read(&self.path)?).as_slice() == self.digest,
            "Owner configuration changed; open a fresh invocation"
        );
        Ok(())
    }

    pub fn receipt(&self, root: &str, relative: &str) -> Result<PathBuf> {
        self.storage_path(root, relative, false)
    }

    pub fn directory(&self, root: &str, relative: &str) -> Result<PathBuf> {
        self.storage_path(root, relative, true)
    }

    fn storage_path(&self, root: &str, relative: &str, directory: bool) -> Result<PathBuf> {
        let root = self
            .config
            .roots
            .get(root)
            .context("Unenrolled storage root")?;
        protected_directory(root)?;
        let relative = Path::new(relative);
        ensure!(
            !relative.as_os_str().is_empty()
                && relative
                    .components()
                    .all(|c| matches!(c, Component::Normal(_))),
            "Receipt must be a nonempty relative path without traversal"
        );
        #[cfg(windows)]
        ensure!(
            !relative.to_string_lossy().contains(':'),
            "Alternate data streams are not receipt paths"
        );
        let path = root.join(relative);
        path_chain(&path)?;
        // Parents are prepared by the owner, never recursively invented by a request.
        protected_directory(path.parent().context("Receipt parent is missing")?)?;
        if path.exists() {
            if directory {
                protected_directory(&path)?;
            } else {
                protected_file(&path)?;
            }
        }
        Ok(path)
    }

    pub fn command(
        &self,
        name: &str,
        result: Option<&str>,
    ) -> Result<(CommandProfile, Vec<String>)> {
        let profile = self
            .config
            .commands
            .get(name)
            .context("Unenrolled command")?
            .clone();
        let replacement = match (&profile.result_root, result) {
            (Some(root), Some(relative)) => Some(
                self.receipt(root, relative)?
                    .into_os_string()
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("Result path is not UTF-8"))?,
            ),
            (None, None) => None,
            _ => anyhow::bail!("Command result argument does not match its enrollment"),
        };
        let args = profile
            .args
            .iter()
            .map(|s| {
                if s == "{result}" {
                    replacement.clone().expect("validated placeholder")
                } else {
                    s.clone()
                }
            })
            .collect();
        Ok((profile, args))
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

/// Reject links/reparse ancestors. The enrolled owner itself is trusted; this is
/// not a sandbox against another process with the same owner's write access.
pub fn path_chain(path: &Path) -> Result<()> {
    ensure!(path.is_absolute(), "Installation paths must be absolute");
    ensure!(
        path.components()
            .all(|c| !matches!(c, Component::ParentDir | Component::CurDir)),
        "Installation path contains traversal"
    );
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(m) => {
                ensure!(
                    !m.file_type().is_symlink(),
                    "Installation path traverses a symbolic link"
                );
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    ensure!(
                        m.file_attributes() & 0x400 == 0,
                        "Installation path traverses a reparse point"
                    );
                    windows_access(ancestor, false)?;
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    let mode = m.mode();
                    // A private directory under the system sticky temporary root is safe.
                    ensure!(
                        mode & 0o022 == 0 || (m.is_dir() && mode & 0o1000 != 0 && m.uid() == 0),
                        "Installation ancestor {} permits writes by another principal",
                        ancestor.display()
                    );
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && ancestor == path => (),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

pub fn protected_file(path: &Path) -> Result<()> {
    path_chain(path)?;
    ensure!(path.is_file(), "Expected an ordinary protected file");
    protected_owner(path)
}
pub fn protected_directory(path: &Path) -> Result<()> {
    path_chain(path)?;
    ensure!(path.is_dir(), "Expected an existing protected directory");
    protected_owner(path)
}
#[cfg(unix)]
fn protected_owner(path: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let m = fs::symlink_metadata(path)?;
    ensure!(
        m.uid() == unsafe { libc::geteuid() } && m.mode() & 0o077 == 0,
        "Installation file/directory must be private to this user"
    );
    Ok(())
}
#[cfg(windows)]
fn protected_owner(path: &Path) -> Result<()> {
    windows_access(path, true)
}
#[cfg(windows)]
fn windows_access(path: &Path, private: bool) -> Result<()> {
    use std::ptr;
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::{
            ACCESS_ALLOWED_ACE,
            Authorization::{ConvertSidToStringSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT},
            DACL_SECURITY_INFORMATION, GetAce, OWNER_SECURITY_INFORMATION,
        },
    };
    unsafe fn sid_string(sid: *mut core::ffi::c_void) -> Result<String> {
        unsafe {
            let mut out = ptr::null_mut();
            ensure!(
                ConvertSidToStringSidW(sid, &mut out) != 0,
                "Cannot read file principal"
            );
            let mut n = 0;
            while *out.add(n) != 0 {
                n += 1;
            }
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(out, n));
            LocalFree(out.cast());
            Ok(text)
        }
    }
    let wide = amplifier_native_platform::windows_native_path(path)?;
    let user = amplifier_native_platform::current_user_sid()?;
    unsafe {
        let mut owner = ptr::null_mut();
        let mut dacl = ptr::null_mut();
        let mut descriptor = ptr::null_mut();
        let code = GetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            &mut dacl,
            ptr::null_mut(),
            &mut descriptor,
        );
        ensure!(
            code == 0,
            "Cannot inspect protected storage policy ({code})"
        );
        let outcome = (|| -> Result<()> {
            ensure!(
                !owner.is_null() && (!private || sid_string(owner)? == user),
                "Installation must belong to this user"
            );
            ensure!(!dacl.is_null(), "Installation has unrestricted access");
            for i in 0..(*dacl).AceCount {
                let mut ace = ptr::null_mut();
                ensure!(
                    GetAce(dacl, i as u32, &mut ace) != 0,
                    "Cannot inspect access entry"
                );
                let header = &*(ace as *const windows_sys::Win32::Security::ACE_HEADER);
                if header.AceFlags & 0x08 != 0 {
                    continue;
                } // INHERIT_ONLY_ACE
                if header.AceType == 0
                /* ACCESS_ALLOWED_ACE_TYPE */
                {
                    let allowed = &*(ace as *const ACCESS_ALLOWED_ACE);
                    let principal =
                        sid_string((&allowed.SidStart as *const u32).cast_mut().cast())?;
                    // Windows may allow creating new sibling directories at a
                    // system ancestor. Unlike Unix directory write, ADD_FILE /
                    // ADD_SUBDIRECTORY alone cannot replace an existing protected
                    // child. Delete-child, delete, owner/DACL changes and generic
                    // write/all still reject. Enrolled roots remain fully private.
                    let mutation_mask = if path.is_dir() {
                        0x500d0150
                    } else {
                        0x500d0156
                    };
                    ensure!(
                        principal == user
                            || principal == "S-1-5-18"
                            || (!private
                                && (allowed.Mask & mutation_mask == 0
                                    || principal == "S-1-5-32-544"
                                    || principal
                                        == "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464")),
                        "Installation {} permits principal {} access {:#x} (private={})",
                        path.display(),
                        principal,
                        allowed.Mask,
                        private
                    );
                } else {
                    // Refuse unfamiliar object/callback allow forms rather than guess their scope.
                    ensure!(
                        header.AceType == 1,
                        "Unsupported installation access policy"
                    );
                }
            }
            Ok(())
        })();
        LocalFree(descriptor.cast());
        outcome
    }
}
