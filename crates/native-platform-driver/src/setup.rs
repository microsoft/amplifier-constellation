//! One-shot, installation-owned setup before a protected enrollment exists.
//! No command/service dispatch is available through this entrypoint.

use crate::{Outcome, config};
use amplifier_native_platform::{private_create, private_create_dir};
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
