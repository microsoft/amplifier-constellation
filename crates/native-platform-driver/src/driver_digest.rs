//! Full-byte driver identity hashing. No file observations are cached here.

use sha2::{Digest, Sha256};

pub(super) fn sha256(bytes: impl AsRef<[u8]>) -> [u8; 32] {
    let bytes = bytes.as_ref();
    #[cfg(all(windows, target_arch = "aarch64"))]
    {
        with_native(bytes, bcrypt_hash)
    }
    #[cfg(not(all(windows, target_arch = "aarch64")))]
    {
        Sha256::digest(bytes).into()
    }
}

#[cfg(any(test, all(windows, target_arch = "aarch64")))]
fn input_length(length: usize) -> Option<u32> {
    u32::try_from(length).ok()
}

#[cfg(any(test, all(windows, target_arch = "aarch64")))]
fn with_native(bytes: &[u8], hash: impl FnOnce(&[u8], u32, &mut [u8; 32]) -> i32) -> [u8; 32] {
    try_native(bytes, hash).unwrap_or_else(|| Sha256::digest(bytes).into())
}

#[cfg(any(test, all(windows, target_arch = "aarch64")))]
fn try_native(
    bytes: &[u8],
    hash: impl FnOnce(&[u8], u32, &mut [u8; 32]) -> i32,
) -> Option<[u8; 32]> {
    // An oversized input uses portable SHA256; never truncate the native length.
    let length = input_length(bytes.len())?;
    let mut digest = [0; 32];
    // BCryptHash reports STATUS_SUCCESS (zero). Discard output on any other status.
    (hash(bytes, length, &mut digest) == 0).then_some(digest)
}

#[cfg(all(windows, target_arch = "aarch64"))]
fn bcrypt_hash(bytes: &[u8], length: u32, digest: &mut [u8; 32]) -> i32 {
    use windows_sys::Win32::Security::Cryptography::{BCRYPT_SHA256_ALG_HANDLE, BCryptHash};

    // SAFETY: length was checked against this immutable slice. The output is an
    // exclusive 32-byte buffer. No secret selects plain SHA256, not HMAC. The
    // Windows 10+ pseudo handle is shared; it is never configured or closed.
    unsafe {
        BCryptHash(
            BCRYPT_SHA256_ALG_HANDLE,
            std::ptr::null(),
            0,
            bytes.as_ptr(),
            length,
            digest.as_mut_ptr(),
            32,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vectors() -> Vec<(Vec<u8>, &'static str)> {
        vec![
            (
                Vec::new(),
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                b"abc".to_vec(),
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            (
                vec![b'a'; 1_000_000],
                "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0",
            ),
        ]
    }

    fn hex(digest: [u8; 32]) -> String {
        digest.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn known_sha256_vectors() {
        for (bytes, expected) in vectors() {
            assert_eq!(hex(sha256(&bytes)), expected);
        }
    }

    #[test]
    fn native_length_is_checked_without_truncation() {
        assert_eq!(input_length(0), Some(0));
        assert_eq!(input_length(u32::MAX as usize), Some(u32::MAX));
        #[cfg(target_pointer_width = "64")]
        {
            assert_eq!(input_length(u32::MAX as usize + 1), None);
            assert_eq!(input_length(usize::MAX), None);
        }
    }

    #[test]
    fn native_receives_exact_full_buffer_and_length() {
        let bytes: Vec<u8> = (0..2_000_003).map(|index| (index % 251) as u8).collect();
        let expected: [u8; 32] = Sha256::digest(&bytes).into();
        let actual = with_native(&bytes, |input, length, digest| {
            assert_eq!(input.as_ptr(), bytes.as_ptr());
            assert_eq!(input, bytes);
            assert_eq!(length as usize, bytes.len());
            *digest = expected;
            0
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn native_error_discards_output_and_hashes_same_full_buffer() {
        let bytes: Vec<u8> = (0..2_000_003).map(|index| (index % 251) as u8).collect();
        let expected: [u8; 32] = Sha256::digest(&bytes).into();
        for status in [-1, i32::MIN, 1] {
            assert_eq!(
                with_native(&bytes, |input, length, digest| {
                    assert_eq!(input.as_ptr(), bytes.as_ptr());
                    assert_eq!(length as usize, bytes.len());
                    digest.fill(0x5a);
                    status
                }),
                expected
            );
        }
    }

    #[test]
    fn complete_binary_and_same_length_drift_match_portable_sha256() {
        let mut bytes = std::fs::read(std::env::current_exe().unwrap()).unwrap();
        let expected: [u8; 32] = Sha256::digest(&bytes).into();
        assert_eq!(sha256(&bytes), expected);
        let last = bytes.last_mut().unwrap();
        *last ^= 1;
        let changed = sha256(&bytes);
        assert_ne!(changed, expected);
        assert_eq!(changed, <[u8; 32]>::from(Sha256::digest(&bytes)));
    }

    // This must run on the regular native Windows ARM target. A fallback-only
    // pass cannot qualify the native implementation.
    #[cfg(all(windows, target_arch = "aarch64"))]
    #[test]
    fn windows_arm_native_success_matches_vectors_and_complete_binary() {
        for (bytes, expected) in vectors() {
            let digest = try_native(&bytes, bcrypt_hash).expect("BCryptHash must succeed natively");
            assert_eq!(hex(digest), expected);
        }
        let bytes = std::fs::read(std::env::current_exe().unwrap()).unwrap();
        let native =
            try_native(&bytes, bcrypt_hash).expect("native full-binary SHA256 must succeed");
        assert_eq!(native, <[u8; 32]>::from(Sha256::digest(&bytes)));
        assert_eq!(sha256(&bytes), native);
    }
}
