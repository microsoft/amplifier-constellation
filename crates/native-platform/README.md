# Native application support

Optional Rust implementation support for self-contained Constellation applications on Linux, macOS and Windows. This crate performs local per-user storage, startup registration and desktop launch actions. It provides no networking, enrollment, identity service or remote commands.

`OBSERVATORY` and `MACHINE_HELPER` are separate fixed application profiles. Startup uses the current user's systemd service, LaunchAgent or least-privilege Task Scheduler entry. Existing mismatched entries and binaries are preserved. Registering startup does not start another process immediately.

Windows task inspection uses Task Scheduler's Unicode XML API, so desktop apps without a console preserve non-ASCII account and installation paths. The same exact task identity checks apply before reuse or removal; an inspection error is not treated as a missing task.

`private_write` commits a fully written protected temporary file; `private_create` additionally refuses any existing destination. Windows storage grants only the current user and SYSTEM; Unix directories and files are 0700 and 0600. Applications retain their state schemas, migrations and credentials.

Optional feature `installation` adds lifetime locks compatible with existing Unix
`flock`, native process generations, and observation/compare-before-dispatch for
enrolled systemd user services, LaunchAgents and Scheduled Tasks. Start/stop
acceptance is distinct from settlement; applications retain admission, transaction
and restoration decisions. No helper enables or unmasks a paused service. The
[local driver](../native-platform-driver/PROTOCOL.md) exposes these primitives to
the [thin Python binding](../../support/python/constellation_native/README.md).
It owns no scheduler or network listener. Managed source-directory/pointer exchange
uses [two explicit native renames](../native-platform-driver/PROTOCOL.md), with an
observable gap. It is separate from protected JSON writes and is not an atomic
app/venv swap; the domain owns the transaction journal, verification and rollback.

`open_browser` accepts only a loopback HTTP/HTTPS setup URL. `show_launch_error` uses a native desktop alert when available. A separate `open_https_browser` can open an application’s already configured HTTPS origin, without embedded credentials, paths, query strings or fragments. Neither function accepts shell source.

Optional feature `local-auth` supplies the existing Argon2id password profile and
domain-separated, nonce-bound HMAC listener proofs. Applications retain their own
credential files, sessions, cookies, rate limits and HTTP boundaries; this adds no
shared account or identity service. Listener proof must be checked against the
expected installation and origin before credential dispatch on the same connection.
The computer helper does not enable this feature.

Run `cargo test --locked` and `cargo clippy --locked --all-targets -- -D warnings`. The ignored startup test additionally writes/removes uniquely named current-user registrations and requires `CONSTELLATION_STARTUP_SMOKE=1` on a disposable runner. Windows x64 hosted CI uses Windows Server; desktop first-run qualification is separate from that result. Windows ARM CI uses Windows 11 ARM.
