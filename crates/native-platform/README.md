# Native application support

Optional Rust implementation support for self-contained Constellation applications on Linux, macOS and Windows. This crate performs local per-user storage, startup registration and desktop launch actions. It provides no networking, enrollment, identity service or remote commands.

`OBSERVATORY` and `MACHINE_HELPER` are separate fixed application profiles. Startup uses the current user's systemd service, LaunchAgent or least-privilege Task Scheduler entry. Existing mismatched entries and binaries are preserved. Registering startup does not start another process immediately.

`private_write` commits a fully written protected temporary file; `private_create` additionally refuses any existing destination. Windows storage grants only the current user and SYSTEM; Unix directories and files are 0700 and 0600. Applications still own locking, state schemas, migrations and credentials.

`open_browser` accepts only a loopback HTTP/HTTPS setup URL. `show_launch_error` uses a native desktop alert when available. No remote site or shell source is accepted.

Run `cargo test --locked` and `cargo clippy --locked --all-targets -- -D warnings`. The ignored startup test additionally writes/removes uniquely named current-user registrations and requires `CONSTELLATION_STARTUP_SMOKE=1` on a disposable runner. Windows x64 hosted CI uses Windows Server; desktop first-run qualification is separate from that result. Windows ARM CI uses Windows 11 ARM.
