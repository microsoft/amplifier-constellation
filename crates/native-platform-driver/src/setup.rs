//! One-shot, installation-owned setup before a protected enrollment exists.
//! No command/service dispatch is available through this entrypoint.

use crate::{Outcome, config};
use amplifier_native_platform::{
    MAX_PRIVATE_EXECUTABLE_BYTES, PrivateExecutableSnapshot, private_create, private_create_dir,
    private_create_executable, private_executable_snapshot,
};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
};

const MAX_SETUP_FRAME: usize = 2 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(tag = "op", deny_unknown_fields)]
enum Request {
    #[serde(rename = "setup.root")]
    Root {
        version: u32,
        id: String,
        path: PathBuf,
    },
    #[serde(rename = "setup.config")]
    Config {
        version: u32,
        id: String,
        path: PathBuf,
        config: Value,
    },
    #[serde(rename = "setup.driver")]
    Driver {
        version: u32,
        id: String,
        path: PathBuf,
        sha256: String,
    },
}

/// The one-shot caller closes stdin after its one bounded JSONL request.
pub fn run() -> Result<()> {
    run_io(io::stdin().lock(), io::stdout().lock())
}

fn run_io(input: impl Read, mut output: impl Write) -> Result<()> {
    let mut frame = Vec::new();
    input
        .take(MAX_SETUP_FRAME as u64 + 1)
        .read_to_end(&mut frame)?;
    let reply = if frame.len() > MAX_SETUP_FRAME || frame.last() != Some(&b'\n') {
        Outcome::failure(
            "refused",
            "invalid_request",
            "Invalid or oversized setup frame",
        )
        .reply("")
    } else {
        match serde_json::from_slice::<Value>(&frame) {
            Ok(value) => dispatch(value),
            Err(error) => Outcome::failure("refused", "invalid_request", error).reply(""),
        }
    };
    serde_json::to_writer(&mut output, &reply)?;
    output.write_all(b"\n")?;
    output.flush()?;
    Ok(())
}

fn dispatch(value: Value) -> Value {
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let outcome = match serde_json::from_value::<Request>(value) {
        Ok(Request::Root { version, id, path }) if valid_request(version, &id) => root(&path),
        Ok(Request::Config {
            version,
            id,
            path,
            config,
        }) if valid_request(version, &id) => save_config(&path, &config),
        Ok(Request::Driver {
            version,
            id,
            path,
            sha256,
        }) if valid_request(version, &id) => save_driver(&path, &sha256),
        Ok(_) => Outcome::failure(
            "refused",
            "invalid_request",
            "Unsupported version or invalid request identity",
        ),
        Err(error) => Outcome::failure("refused", "invalid_request", error),
    };
    outcome.reply(&id)
}

fn valid_request(version: u32, id: &str) -> bool {
    version == 1 && !id.is_empty() && id.len() <= 128
}

fn destination(path: &Path) -> Result<&Path> {
    ensure!(path.is_absolute(), "Setup path must be absolute");
    ensure!(
        path.components()
            .all(|component| !matches!(component, Component::ParentDir | Component::CurDir)),
        "Setup path contains traversal"
    );
    ensure!(path.file_name().is_some(), "Setup path needs a final name");
    #[cfg(windows)]
    ensure!(
        path.components().all(|component| match component {
            Component::Normal(name) => !name.to_string_lossy().contains(':'),
            _ => true,
        }),
        "Alternate data streams are not setup paths"
    );
    let parent = path
        .parent()
        .context("Setup path needs an existing parent")?;
    config::path_chain(parent)?;
    ensure!(
        parent.is_dir(),
        "Setup parent must be an existing ordinary directory"
    );
    config::path_chain(path)?;
    Ok(parent)
}

fn preflight_error(error: anyhow::Error) -> Outcome {
    let storage_error = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<io::Error>());
    if storage_error.is_some_and(|error| error.kind() != io::ErrorKind::NotFound) {
        Outcome::failure("error", "setup_observation_failed", error)
    } else {
        Outcome::failure("refused", "invalid_setup", error)
    }
}

fn root(path: &Path) -> Outcome {
    let parent = match destination(path) {
        Ok(parent) => parent,
        Err(error) => return preflight_error(error),
    };
    match fs::symlink_metadata(path) {
        Ok(_) => return existing_root(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return preflight_error(error.into()),
    }
    match private_create_dir(path) {
        Ok(()) => {
            let verified = (|| -> Result<()> {
                config::protected_directory(path)?;
                #[cfg(unix)]
                fs::File::open(parent)?.sync_all()?;
                #[cfg(windows)]
                let _ = parent;
                Ok(())
            })();
            match verified {
                Ok(()) => Outcome::ok(json!({"path":path,"created":true})),
                Err(error) => Outcome::failure("unknown", "setup_root_unconfirmed", error),
            }
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => existing_root(path),
        Err(error) => Outcome::failure("unknown", "setup_root_unconfirmed", error),
    }
}

fn existing_root(path: &Path) -> Outcome {
    match config::protected_directory(path) {
        Ok(()) => Outcome::ok(json!({"path":path,"created":false})),
        Err(error) => preflight_error(error),
    }
}

fn save_config(path: &Path, value: &Value) -> Outcome {
    let bytes = match serde_json::to_vec(value) {
        Ok(bytes) => bytes,
        Err(error) => return preflight_error(error.into()),
    };
    let validated = (|| -> Result<()> {
        let parent = destination(path)?;
        config::protected_directory(parent)?;
        config::Enrollment::from_bytes(path, &bytes)?;
        Ok(())
    })();
    if let Err(error) = validated {
        return preflight_error(error);
    }
    match fs::symlink_metadata(path) {
        Ok(_) => return existing_config(path, &bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return preflight_error(error.into()),
    }
    match private_create(path, &bytes) {
        Ok(()) => match verify_config(path, &bytes) {
            Ok(()) => Outcome::ok(config_result(path, &bytes, true)),
            Err(error) => Outcome::failure("unknown", "setup_config_unconfirmed", error),
        },
        Err(error)
            if error.chain().any(|cause| {
                cause
                    .downcast_ref::<io::Error>()
                    .is_some_and(|error| error.kind() == io::ErrorKind::AlreadyExists)
            }) =>
        {
            existing_config(path, &bytes)
        }
        Err(error) => Outcome::failure("unknown", "setup_config_unconfirmed", error),
    }
}

fn existing_config(path: &Path, expected: &[u8]) -> Outcome {
    match verify_config(path, expected) {
        Ok(()) => Outcome::ok(config_result(path, expected, false)),
        Err(error) => preflight_error(error),
    }
}

fn verify_config(path: &Path, expected: &[u8]) -> Result<()> {
    config::Enrollment::load(path)?;
    ensure!(
        fs::read(path)? == expected,
        "Existing owner configuration differs"
    );
    Ok(())
}

fn config_result(path: &Path, bytes: &[u8], created: bool) -> Value {
    json!({"path":path,"created":created,"sha256":format!("{:x}",Sha256::digest(bytes))})
}

fn save_driver(path: &Path, expected: &str) -> Outcome {
    let prepared = (|| -> Result<(fs::File, [u8; 32], u64)> {
        ensure!(
            expected.len() == 64
                && expected
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "Expected driver SHA256 must be 64 lowercase hexadecimal characters"
        );
        let parent = destination(path)?;
        config::protected_directory(parent)?;
        // Only this executing driver can be installed. Keep the verified source
        // handle open through publication; never hash then reopen a source path.
        let current = std::env::current_exe()?;
        let metadata = fs::symlink_metadata(&current)?;
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "Running driver must be an ordinary file"
        );
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            ensure!(
                metadata.file_attributes() & 0x400 == 0,
                "Running driver is a reparse point"
            );
        }
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::{
                FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ,
            };
            options
                .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
                .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        }
        let mut source = options.open(current)?;
        let before = source.metadata()?;
        ensure!(
            before.is_file() && before.len() > 0 && before.len() <= MAX_PRIVATE_EXECUTABLE_BYTES,
            "Running driver size is invalid"
        );
        let mut hasher = Sha256::new();
        let mut size = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = source.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            size += count as u64;
            ensure!(
                size <= MAX_PRIVATE_EXECUTABLE_BYTES,
                "Running driver exceeds size limit"
            );
            hasher.update(&buffer[..count]);
        }
        let digest: [u8; 32] = hasher.finalize().into();
        let after = source.metadata()?;
        ensure!(
            size == before.len()
                && size == after.len()
                && before.modified()? == after.modified()?,
            "Running driver changed during validation"
        );
        let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        ensure!(
            rendered == expected,
            "Running driver differs from expected SHA256"
        );
        Ok((source, digest, size))
    })();
    let (mut source, digest, size) = match prepared {
        Ok(value) => value,
        Err(error) => return preflight_error(error),
    };
    match fs::symlink_metadata(path) {
        Ok(_) => return existing_driver(path, &digest, size),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return preflight_error(error.into()),
    }
    match private_create_executable(path, &mut source, &digest, size) {
        Ok(observed) => match config::protected_file(path) {
            Ok(()) => Outcome::ok(driver_result(path, &observed, true)),
            Err(error) => Outcome::failure("unknown", "setup_driver_unconfirmed", error),
        },
        Err(error)
            if error.chain().any(|cause| {
                cause
                    .downcast_ref::<io::Error>()
                    .is_some_and(|e| e.kind() == io::ErrorKind::AlreadyExists)
            }) =>
        {
            existing_driver(path, &digest, size)
        }
        Err(error) => Outcome::failure("unknown", "setup_driver_unconfirmed", error),
    }
}

fn existing_driver(path: &Path, expected: &[u8; 32], size: u64) -> Outcome {
    let observed = (|| -> Result<PrivateExecutableSnapshot> {
        config::protected_file(path)?;
        let observed = private_executable_snapshot(path)?;
        ensure!(
            observed.sha256 == *expected && observed.size == size,
            "Existing private driver differs"
        );
        config::protected_file(path)?;
        Ok(observed)
    })();
    match observed {
        Ok(observed) => Outcome::ok(driver_result(path, &observed, false)),
        Err(error) => preflight_error(error),
    }
}

fn driver_result(path: &Path, observed: &PrivateExecutableSnapshot, created: bool) -> Value {
    let sha256: String = observed
        .sha256
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    json!({"path":path,"created":created,"sha256":sha256,"size":observed.size,"identity":observed.identity})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary() -> tempfile::TempDir {
        match std::env::var_os("CONSTELLATION_NATIVE_TEST_ROOT") {
            Some(root) => tempfile::tempdir_in(root).unwrap(),
            None => tempfile::tempdir().unwrap(),
        }
    }
    fn request(op: &str, path: &Path, config: Option<Value>) -> Value {
        let mut value = json!({"version":1,"id":"setup-fixture","op":op,"path":path});
        if let Some(config) = config {
            value["config"] = config;
        }
        dispatch(value)
    }
    fn root_request(path: &Path) -> Value {
        request("setup.root", path, None)
    }
    fn config_request(path: &Path, root: &Path) -> Value {
        request(
            "setup.config",
            path,
            Some(json!({"version":1,"roots":{"state":root}})),
        )
    }

    fn driver_request(path: &Path, sha256: &str) -> Value {
        dispatch(
            json!({"version":1,"id":"driver-fixture","op":"setup.driver","path":path,"sha256":sha256}),
        )
    }

    fn running_digest() -> String {
        format!(
            "{:x}",
            Sha256::digest(fs::read(std::env::current_exe().unwrap()).unwrap())
        )
    }

    #[test]
    fn driver_bootstrap_installs_only_running_image_and_repeat_is_read_only() {
        let temporary = temporary();
        let parent = temporary.path().canonicalize().unwrap().join("private");
        assert_eq!(root_request(&parent)["outcome"], "ok");
        let path = parent.join("driver");
        let expected = running_digest();
        let created = driver_request(&path, &expected);
        assert_eq!(created["outcome"], "ok", "{created}");
        assert_eq!(created["result"]["created"], true);
        assert_eq!(created["result"]["sha256"], expected);
        assert!(created["result"]["identity"].is_object());
        assert_eq!(
            fs::read(&path).unwrap(),
            fs::read(std::env::current_exe().unwrap()).unwrap()
        );
        let metadata = fs::metadata(&path).unwrap();
        let repeated = driver_request(&path, &expected);
        assert_eq!(repeated["outcome"], "ok", "{repeated}");
        assert_eq!(repeated["result"]["created"], false);
        assert_eq!(
            repeated["result"]["identity"],
            created["result"]["identity"]
        );
        assert_eq!(
            fs::metadata(&path).unwrap().modified().unwrap(),
            metadata.modified().unwrap()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(metadata.permissions().mode() & 0o7777, 0o700);
        }
    }

    #[test]
    fn driver_bootstrap_wrong_digest_unknown_source_and_conflict_do_not_replace() {
        let temporary = temporary();
        let parent = temporary.path().canonicalize().unwrap().join("private");
        assert_eq!(root_request(&parent)["outcome"], "ok");
        let path = parent.join("driver");
        for digest in ["0".repeat(64), "A".repeat(64), "short".into()] {
            assert_eq!(driver_request(&path, &digest)["outcome"], "refused");
            assert!(!path.exists());
        }
        let expected = running_digest();
        let request = json!({"version":1,"id":"fixture","op":"setup.driver","path":path,"sha256":expected,"source":std::env::current_exe().unwrap()});
        assert_eq!(dispatch(request)["outcome"], "refused");
        assert!(!path.exists());
        private_create(&path, b"preserve conflicting owner file").unwrap();
        let before = fs::metadata(&path).unwrap();
        assert_eq!(driver_request(&path, &expected)["outcome"], "refused");
        assert_eq!(fs::read(&path).unwrap(), b"preserve conflicting owner file");
        assert_eq!(
            fs::metadata(&path).unwrap().modified().unwrap(),
            before.modified().unwrap()
        );
        let absent_parent = parent.join("missing");
        assert_eq!(
            driver_request(&absent_parent.join("driver"), &expected)["outcome"],
            "refused"
        );
        assert!(!absent_parent.exists());
    }

    #[cfg(unix)]
    #[test]
    fn driver_bootstrap_equal_public_or_nonexecutable_files_are_not_repaired() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
        let temporary = temporary();
        let parent = temporary.path().canonicalize().unwrap().join("private");
        assert_eq!(root_request(&parent)["outcome"], "ok");
        let path = parent.join("driver");
        let expected = running_digest();
        assert_eq!(driver_request(&path, &expected)["outcome"], "ok");
        for mode in [0o755, 0o600, 0o4700] {
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
            let before = fs::metadata(&path).unwrap();
            assert_eq!(driver_request(&path, &expected)["outcome"], "refused");
            let after = fs::metadata(&path).unwrap();
            assert_eq!(after.mode(), before.mode());
            assert_eq!(after.ino(), before.ino());
            assert_eq!(after.modified().unwrap(), before.modified().unwrap());
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o500)).unwrap();
        assert_eq!(driver_request(&path, &expected)["result"]["created"], false);
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o7777, 0o500);
        let link = parent.join("link");
        symlink(&path, &link).unwrap();
        assert_eq!(driver_request(&link, &expected)["outcome"], "refused");
        assert_eq!(fs::read_link(&link).unwrap(), path);
        let public_parent = parent.join("public");
        fs::create_dir(&public_parent).unwrap();
        fs::set_permissions(&public_parent, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            driver_request(&public_parent.join("driver"), &expected)["outcome"],
            "refused"
        );
        assert_eq!(fs::metadata(&public_parent).unwrap().mode() & 0o7777, 0o755);
    }

    #[test]
    fn fresh_root_and_config_are_private_and_exact_repeats_do_not_replace() {
        let temporary = temporary();
        let parent = temporary.path().canonicalize().unwrap();
        let root = parent.join("owner-private");
        assert_eq!(root_request(&root)["result"]["created"], true);
        config::protected_directory(&root).unwrap();
        assert_eq!(root_request(&root)["result"]["created"], false);
        let file = root.join("owner.json");
        let created = config_request(&file, &root);
        assert_eq!(created["result"]["created"], true);
        config::Enrollment::load(&file).unwrap();
        let before = fs::read(&file).unwrap();
        let before_metadata = fs::metadata(&file).unwrap();
        let repeated = config_request(&file, &root);
        assert_eq!(repeated["result"]["created"], false);
        assert_eq!(repeated["result"]["sha256"], created["result"]["sha256"]);
        assert_eq!(fs::read(&file).unwrap(), before);
        assert_eq!(
            fs::metadata(&file).unwrap().modified().unwrap(),
            before_metadata.modified().unwrap()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(fs::metadata(&file).unwrap().ino(), before_metadata.ino());
        }
        let changed = request("setup.config", &file, Some(json!({"version":1})));
        assert_eq!(changed["outcome"], "refused");
        assert_eq!(fs::read(&file).unwrap(), before);
    }

    #[test]
    fn missing_parent_invalid_config_and_non_directory_root_do_not_mutate() {
        let temporary = temporary();
        let parent = temporary.path().canonicalize().unwrap();
        let missing = parent.join("missing");
        assert_eq!(root_request(&missing.join("child"))["outcome"], "refused");
        assert!(!missing.exists());
        let root = parent.join("private");
        assert_eq!(root_request(&root)["outcome"], "ok");
        let file = root.join("owner.json");
        for invalid in [
            json!({"version":2}),
            json!({"version":1,"unexpected":true}),
            json!({"version":1,"roots":{"absent":missing}}),
        ] {
            assert_eq!(
                request("setup.config", &file, Some(invalid))["outcome"],
                "refused"
            );
            assert!(!file.exists());
        }
        private_create(&file, b"unrelated existing bytes").unwrap();
        assert_eq!(root_request(&file)["outcome"], "refused");
        assert_eq!(fs::read(&file).unwrap(), b"unrelated existing bytes");
    }

    #[cfg(unix)]
    #[test]
    fn existing_public_root_and_links_refuse_without_chmod_or_replacement() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let temporary = temporary();
        let parent = temporary.path().canonicalize().unwrap();
        let root = parent.join("existing");
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        let before = fs::metadata(&root).unwrap().permissions().mode();
        assert_eq!(root_request(&root)["outcome"], "refused");
        assert_eq!(
            config_request(&root.join("owner.json"), &root)["outcome"],
            "refused"
        );
        assert_eq!(fs::metadata(&root).unwrap().permissions().mode(), before);
        assert!(!root.join("owner.json").exists());
        let link = parent.join("link");
        symlink(&root, &link).unwrap();
        assert_eq!(root_request(&link)["outcome"], "refused");
        assert_eq!(
            config_request(&link.join("owner.json"), &root)["outcome"],
            "refused"
        );
        assert_eq!(fs::read_link(&link).unwrap(), root);
    }

    #[cfg(unix)]
    #[test]
    fn fresh_child_preserves_safe_readable_parent_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let temporary = temporary();
        let parent = temporary.path().canonicalize().unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
        let before = fs::metadata(&parent).unwrap().permissions().mode();
        let child = parent.join("owned");
        assert_eq!(root_request(&child)["outcome"], "ok");
        assert_eq!(
            config_request(&child.join("owner.json"), &child)["outcome"],
            "ok"
        );
        assert_eq!(fs::metadata(&parent).unwrap().permissions().mode(), before);
        config::protected_directory(&child).unwrap();
    }

    #[test]
    fn unknown_fields_multiple_frames_and_oversize_refuse_before_setup() {
        let temporary = temporary();
        let path = temporary
            .path()
            .canonicalize()
            .unwrap()
            .join("never-created");
        let mut request = json!({"version":1,"id":"fixture","op":"setup.root","path":path,"command":"not-supported"});
        assert_eq!(dispatch(request.clone())["outcome"], "refused");
        request.as_object_mut().unwrap().remove("command");
        let line = format!("{request}\n{request}\n");
        let mut output = Vec::new();
        run_io(line.as_bytes(), &mut output).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&output).unwrap()["outcome"],
            "refused"
        );
        output.clear();
        run_io(vec![b'x'; MAX_SETUP_FRAME + 1].as_slice(), &mut output).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&output).unwrap()["outcome"],
            "refused"
        );
        assert!(!path.exists());
    }
}
