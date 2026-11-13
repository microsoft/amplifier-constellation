#ifndef CONSTELLATION_NATIVE_SESSION_H
#define CONSTELLATION_NATIVE_SESSION_H
#include <stddef.h>
#include <stdint.h>

/* ABI 1. All text is UTF-8. No function accepts a transport timeout.
 * Buffers are owned by this library; copy their bytes then free exactly once.
 * Keep the library loaded until every session and buffer has been released.
 * Once fork callbacks are registered, keep it loaded for the process lifetime:
 * Python cannot unregister those callbacks when the last Session closes.
 * Return 0 means an outcome JSON was written, not that the operation succeeded.
 * Return 1 means invalid ABI arguments; 2 means an outcome could not be encoded.
 * An invalid/unknown result never authorizes retry of a mutating operation.
 * Inputs must be immutable throughout each call. Input/output memory ranges
 * (including out_handle) must not overlap. Output storage must be writable and
 * aligned. A non-null output buffer is zeroed even on invalid-argument return.
 */
typedef struct { uint8_t *data; size_t len; } constellation_native_buffer;

uint32_t constellation_native_abi_version(void);

/* options: {"version":1,"config":"absolute protected config path",
 *           "driver":"absolute protected installed executable path"}
 * On success, out_handle is nonzero. On refusal it is zero.
 * Reply uses existing version/outcome/result/error fields, id="native.open".
 */
int32_t constellation_native_open(const uint8_t *options, size_t len,
    uint64_t *out_handle, constellation_native_buffer *out);

/* Existing protocol-1 JSON request/response, bounded to 16 MiB per request.
 * Locks are in this process. Calls from another thread while a session owns
 * locks refuse busy; the Python wrapper serializes its entire critical scope.
 */
int32_t constellation_native_request(uint64_t handle, const uint8_t *request,
    size_t len, constellation_native_buffer *out);

/* Refuses busy while any lock is held; never silently unlocks caller work.
 * Release named locks first. A poisoned session still permits lock.release.
 * Reply id="native.close". Re-closing a closed handle refuses invalid_handle.
 */
int32_t constellation_native_close(uint64_t handle, constellation_native_buffer *out);
void constellation_native_buffer_free(constellation_native_buffer buffer);

/* Register these EXACTLY ONCE per loaded library image (not per Session) with
 * Python os.register_at_fork, respectively before,
 * after_in_parent, after_in_child. They coordinate all sessions in this library.
 * A Python-returning fork child exits 125 if ANY native lock was held.
 * Otherwise inherited handles are invalidated; new child sessions are allowed.
 * Never use preexec_fn in a native critical section. Ordinary subprocess
 * fork/exec is supported: descriptors are noninheritable, not passed explicitly.
 * Foreign forks that continue running without these hooks are unsupported.
 * These callbacks are no-ops on Windows. Do not call them as general operations,
 * recursively, or from active ABI dispatch. Each prepare requires one matching
 * callback in parent and child before any further native operation.
 */
void constellation_native_fork_prepare(void);
void constellation_native_fork_parent(void);
void constellation_native_fork_child(void);
#endif
