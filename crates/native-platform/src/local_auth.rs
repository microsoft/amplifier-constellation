//! Optional cryptographic primitives for installation-owned local accounts.
//! No storage, HTTP, session, identity registry or OS authentication is provided.
use anyhow::{Result, ensure};
use argon2::{
    Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub fn validate_password(password: &str) -> Result<()> {
    ensure!(
        password.chars().count() >= 12 && password.len() <= 1024 && !password.contains('\0'),
        "Choose a password with at least 12 characters (up to 1024 bytes)"
    );
    Ok(())
}

/// Produce the existing local-account Argon2id v19, 19MiB/t2/p1 PHC profile.
/// The caller saves it create-only within its own protected installation.
pub fn hash_password(password: &str) -> Result<String> {
    use rand_core::RngCore;
    validate_password(password)?;
    let mut salt_bytes = [0u8; 16];
    rand_core::OsRng
        .try_fill_bytes(&mut salt_bytes)
        .map_err(|_| anyhow::anyhow!("Secure randomness is unavailable"))?;
    let salt = SaltString::encode_b64(&salt_bytes).expect("fixed password salt size");
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| anyhow::anyhow!("Could not protect the password"))
}

/// A malformed or changed profile must not select another authenticator or
/// request unbounded hashing work. The file schema remains the caller's concern.
pub fn verify_password(encoded: &str, password: &str) -> bool {
    if encoded.len() > 4096 || password.len() > 1024 || password.contains('\0') {
        return false;
    }
    let Ok(hash) = PasswordHash::new(encoded) else {
        return false;
    };
    let expected = Params::new(19 * 1024, 2, 1, Some(32)).expect("fixed local-account profile");
    hash.algorithm.as_str() == "argon2id"
        && hash.version == Some(19)
        && Params::try_from(&hash).is_ok_and(|params| params == expected)
        && hash.salt.is_some_and(|salt| salt.as_str().len() == 22)
        && hash.hash.is_some_and(|output| output.len() == 32)
        && Argon2::default()
            .verify_password(password.as_bytes(), &hash)
            .is_ok()
}

fn hex_256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn proof_mac(
    domain: &str,
    id: &str,
    origin: &str,
    nonce: &str,
    secret: &str,
) -> Result<Hmac<Sha256>> {
    for (value, maximum) in [(domain, 128), (id, 128), (origin, 2048)] {
        ensure!(
            !value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control),
            "Invalid local listener proof binding"
        );
    }
    ensure!(
        hex_256(nonce),
        "Use a 256-bit lowercase hexadecimal challenge"
    );
    ensure!(
        (32..=4096).contains(&secret.len()),
        "Invalid local listener credential"
    );
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts text keys");
    mac.update(format!("{domain}\n{id}\n{origin}\n{nonce}").as_bytes());
    Ok(mac)
}

/// Prove possession of a saved installation credential without transmitting it.
/// This authenticates no caller. Each application supplies its own fixed domain.
pub fn listener_proof(
    domain: &str,
    id: &str,
    origin: &str,
    nonce: &str,
    secret: &str,
) -> Result<String> {
    Ok(proof_mac(domain, id, origin, nonce, secret)?
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// Constant-time MAC comparison. The caller also validates response metadata and
/// keeps proof and credential dispatch on the same connection, without reconnect.
pub fn verify_listener_proof(
    domain: &str,
    id: &str,
    origin: &str,
    nonce: &str,
    secret: &str,
    proof: &str,
) -> Result<()> {
    ensure!(hex_256(proof), "Invalid local listener proof");
    let bytes = (0..32)
        .map(|i| u8::from_str_radix(&proof[2 * i..2 * i + 2], 16).expect("validated hexadecimal"))
        .collect::<Vec<_>>();
    proof_mac(domain, id, origin, nonce, secret)?
        .verify_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("Local listener proof did not match"))
}
