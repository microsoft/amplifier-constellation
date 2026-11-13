use super::*;
use amplifier_native_platform::{private_dir, private_write};

fn open(options: &[u8], handle: &mut u64) -> Value {
    let _gate = Gate::acquire();
    super::open(options, handle)
}
fn request(handle: u64, frame: &[u8]) -> Value {
    let _gate = Gate::acquire();
    super::request(handle, frame)
}
fn close(handle: u64) -> Value {
    let _gate = Gate::acquire();
    super::close(handle)
}
fn with_registry(id: &str, f: impl FnOnce(&mut Registry) -> Value) -> Value {
    let _gate = Gate::acquire();
    super::with_registry(id, f)
}

fn fixture() -> (tempfile::TempDir, Vec<u8>) {
    let directory = match std::env::var_os("CONSTELLATION_NATIVE_TEST_ROOT") {
        Some(root) => tempfile::tempdir_in(root).unwrap(),
        None => tempfile::tempdir().unwrap(),
    };
    let home = directory.path().canonicalize().unwrap();
    private_dir(&home).unwrap();
    let driver = home.join("fixed-driver");
    // These tests do not dispatch a process. Driver identity is still protected.
    private_write(&driver, b"fixed fixture driver bytes").unwrap();
    let config = home.join("owner.json");
    private_write(
        &config,
        &serde_json::to_vec(&json!({
            "version":1,"roots":{"state":home},"locks":{"owner":home.join("owner.lock")}
        }))
        .unwrap(),
    )
    .unwrap();
    let options =
        serde_json::to_vec(&json!({"version":1,"config":config,"driver":driver})).unwrap();
    (directory, options)
}

fn lock_request(op: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({"version":1,"id":op,"op":op,"name":"owner"})).unwrap()
}

#[test]
fn panic_retains_real_lock_and_poison_allows_only_valid_release() {
    let (_home, options) = fixture();
    let mut first = 0;
    let mut contender = 0;
    assert_eq!(open(&options, &mut first)["outcome"], "ok");
    assert_eq!(open(&options, &mut contender)["outcome"], "ok");
    assert_eq!(
        request(first, &lock_request("lock.acquire"))["outcome"],
        "ok"
    );
    let result = with_registry("panic", |registry| {
        registry
            .entries
            .get_mut(&first)
            .unwrap()
            .dispatch("panic", |_| panic!("injected dispatch failure"))
    });
    assert_eq!(result["outcome"], "unknown");
    assert_eq!(
        request(contender, &lock_request("lock.acquire"))["outcome"],
        "busy"
    );
    assert_eq!(close(first)["error"]["code"], "locks_held");
    assert_eq!(
        request(first, br#"{"version":1,"id":"describe","op":"describe"}"#)["error"]["code"],
        "session_poisoned"
    );
    // A release-shaped but unrecognized command cannot bypass poisoning.
    assert_eq!(
        request(
            first,
            br#"{"version":1,"id":"bad","op":"lock.release","name":"owner","extra":true}"#
        )["error"]["code"],
        "session_poisoned"
    );
    assert_eq!(
        request(first, &lock_request("lock.release"))["outcome"],
        "ok"
    );
    assert_eq!(
        request(contender, &lock_request("lock.acquire"))["outcome"],
        "ok"
    );
    assert_eq!(
        request(contender, &lock_request("lock.release"))["outcome"],
        "ok"
    );
    assert_eq!(close(first)["outcome"], "ok");
    assert_eq!(close(contender)["outcome"], "ok");
}

#[test]
fn input_limits_and_buffer_ownership_are_explicit() {
    let mut output = NativeBuffer {
        data: ptr::null_mut(),
        len: 0,
    };
    // SAFETY: output is valid and the refused input is deliberately null.
    assert_eq!(
        unsafe { constellation_native_request(0, ptr::null(), 1, &mut output) },
        1
    );
    assert!(output.data.is_null());
    // Empty JSON input produces a normal invalid-handle response without an
    // unchecked memory read. No native operation is dispatched.
    assert_eq!(
        unsafe { constellation_native_request(0, ptr::null(), 0, &mut output) },
        0
    );
    let value: Value =
        serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
            .unwrap();
    assert_eq!(value["error"]["code"], "invalid_handle");
    unsafe {
        constellation_native_buffer_free(output);
    }
}

#[test]
fn acquisition_panic_retains_lock_until_attempted_name_is_released() {
    let (_home, options) = fixture();
    let mut first = 0;
    let mut contender = 0;
    assert_eq!(open(&options, &mut first)["outcome"], "ok");
    assert_eq!(open(&options, &mut contender)["outcome"], "ok");
    let response = with_registry("acquire", |registry| {
        registry
            .entries
            .get_mut(&first)
            .unwrap()
            .dispatch("acquire", |session| {
                assert_eq!(
                    session.request(&lock_request("lock.acquire"))["outcome"],
                    "ok"
                );
                panic!("injected failure before acquisition acknowledgment")
            })
    });
    assert_eq!(response["outcome"], "unknown");
    assert_eq!(
        request(contender, &lock_request("lock.acquire"))["outcome"],
        "busy"
    );
    assert_eq!(close(first)["error"]["code"], "locks_held");
    assert_eq!(
        request(first, &lock_request("lock.release"))["outcome"],
        "ok"
    );
    assert_eq!(
        request(contender, &lock_request("lock.acquire"))["outcome"],
        "ok"
    );
    assert_eq!(
        request(contender, &lock_request("lock.release"))["outcome"],
        "ok"
    );
    assert_eq!(close(first)["outcome"], "ok");
    assert_eq!(close(contender)["outcome"], "ok");
}
