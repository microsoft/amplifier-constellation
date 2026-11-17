# Bounded Windows ARM write diagnostic

The `Native installation driver` workflow has an explicit `write-diagnostic`
dispatch mode. It runs one Windows ARM job; the normal six-target qualification
matrix remains the default. This mode does not stage a consumer package, start a
service or qualify an installed product.

The job builds two distinct release/static-CRT pairs with Rust 1.93.0:

- An uninstrumented control from exact source
  `8062eda22ea4fe9c333798be8397ce7fa4f6f2ee`.
- The dispatched source with the opt-in `native-write-diagnostic` feature.

The feature is disabled by default. It adds clocks around existing checks and
write phases without caching validation, changing durability, retrying calls or
changing outcomes. Each measured dispatch emits one path-free JSON record to
stderr after its measured Rust work. Output failures do not replace the operation
result. The feature is for diagnostics, not for distribution artifacts.

`run_write_diagnostic.py` uses the exact control's three hash-bound Python facade
files and the actual `NativeSession` ABI, including its caller-owned mutex and
native installation lock. Each arm has a separate protected temporary root. The
parent is the current user's profile, or the explicit existing absolute
`CONSTELLATION_NATIVE_TEST_ROOT`. Its resolved path is recorded and bound to
cleanup. Only newly created fixture paths are protected; existing ancestors are
never modified, and normal native enrollment validates their policy. The
fixed workload contains 63 create-only writes: three repetitions of 0, 1 KiB and
64 KiB at each state-relative depth 7, 8, 9, 10, 11, 14 and 15. It interleaves 63
`describe` calls and adds seven refusal checks against existing destinations.
These are stratified samples, not a weighted prediction of whole-package time.
Zero-byte seeds and direct 64 KiB writes cover the native publication boundary;
later filling of large files is outside this fixture.

Each write verifies its returned result, exact bytes, owner and private DACL.
These post-call checks record named observed/expected values and row/path context
before asserting, outside the measurement clocks. Content records contain length
and digest; the actual predicate still compares all bytes. Failures retain a
traceback before cleanup. The exact DACL-text predicate is not normalized.
Duplicate publication must preserve content and file identity. Contention remains
effective throughout the caller's critical section, and the lock becomes
available after release. Payload removal uses the native owner; parent cleanup
only removes its marked temporary root after the child is observed exited. A
timeout records uncertainty, terminates only that owned child and never retries
the operation. Raw intent/result logs and terminal cleanup outcomes are retained.

Timing records distinguish the whole Python method from the original
`NativeSession._call`, whose clock starts after `bytes=list(data)`. Rust phase
times are inclusive. `path.chain` and `path.windows_access` are nested and must
not be added to the top-level driver, enrollment and storage phase sum. The
correlated output reports both the remaining Rust dispatch time and the outer
native-call residual. The latter includes Python JSON, ABI framing/decoding and
post-dispatch diagnostic logging; instrumentation overhead is not subtracted.
An unexplained residual prevents exhaustive phase attribution.

The original failed Windows ARM package retained native hashes but did not upload
those binary bytes. This job records fresh control and timing artifact hashes and
compares the control with the retained hashes without assuming equality. Its
results remain separate from the original full-stage failure. The uploaded
evidence includes both executable/library pairs, compiler/source/flags, raw
caller and phase records, and the bounded completion receipt.
