#![cfg(feature = "installation")]
//! Actual OS-owned operation lifetime; ordinary unit tests never register services.
use std::{path::PathBuf, process::Command};

#[test]
#[ignore = "registers fresh UUID current-user services; explicit disposable native runner required"]
fn os_worker_outlives_parent_and_replacement_outlives_retired_worker() {
    assert_eq!(
        std::env::var("CONSTELLATION_INSTALLATION_SMOKE").as_deref(),
        Ok("1")
    );
    let executable = PathBuf::from(
        std::env::var_os("NATIVE_OPERATION_LIFETIME_FIXTURE")
            .expect("Set the exact built operation_lifetime_fixture executable"),
    );
    assert!(executable.is_absolute() && executable.is_file());
    let output = std::env::var_os("NATIVE_OPERATION_LIFETIME_RESULT")
        .map(PathBuf::from)
        .expect("Set an explicit retained JSON result path");
    assert!(
        output.is_absolute() && !output.exists(),
        "Do not replace a prior qualification receipt"
    );
    let result = Command::new(executable)
        .arg("run")
        .arg(&output)
        .output()
        .unwrap();
    println!("{}", String::from_utf8_lossy(&result.stdout));
    eprintln!("{}", String::from_utf8_lossy(&result.stderr));
    assert!(
        result.status.success(),
        "Actual lifetime fixture failed; result: {}",
        output.display()
    );
    let receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&output).unwrap()).unwrap();
    assert_eq!(receipt["passed"], true);
    assert_eq!(receipt["complete"], true);
    assert_eq!(receipt["cases"].as_array().unwrap().len(), 2);
}
