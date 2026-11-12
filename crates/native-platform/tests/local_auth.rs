#![cfg(feature = "local-auth")]
use amplifier_native_platform::local_auth::*;

// Independent argon2-cffi fixture using the reviewed existing Observatory profile.
const LEGACY: &str = "$argon2id$v=19$m=19456,t=2,p=1$MDEyMzQ1Njc4OWFiY2RlZg$cg9n4FZQD0ige/n82t16+IBP7X5QKwwqVZEE4pJeOYc";
const PASSWORD: &str = "one installation password";
const DOMAIN: &str = "agent-observatory.local-proof/1";
const ID: &str = "12345678-1234-4234-8234-123456789abc";
const ORIGIN: &str = "http://127.0.0.1:49152";
const SECRET: &str = "fixture-control-token-32-characters";

#[test]
fn accepts_existing_password_profile_and_salts_new_hashes() {
    assert!(verify_password(LEGACY, PASSWORD));
    assert!(!verify_password(LEGACY, "different installation password"));
    let a = hash_password(PASSWORD).unwrap();
    let b = hash_password(PASSWORD).unwrap();
    assert_ne!(a, b);
    assert!(verify_password(&a, PASSWORD));
    assert!(verify_password(&b, PASSWORD));
    assert!(a.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
}

#[test]
fn refuses_malformed_or_changed_cost_without_work_or_fallback() {
    for malformed in [
        String::new(),
        "a".repeat(4097),
        LEGACY.replace("argon2id", "argon2i"),
        LEGACY.replace("v=19", "v=16"),
        LEGACY.replace("m=19456", "m=4294967295"),
        LEGACY.replace("t=2", "t=3"),
        LEGACY.replace("p=1", "p=2"),
        LEGACY.replace("MDEyMzQ1Njc4OWFiY2RlZg", "c2hvcnQ"),
        LEGACY.replace("cg9n4FZQD0ige/n82t16+IBP7X5QKwwqVZEE4pJeOYc", "c2hvcnQ"),
    ] {
        assert!(!verify_password(&malformed, PASSWORD));
    }
}

#[test]
fn password_bounds_are_character_minimum_and_byte_maximum() {
    assert!(validate_password(&"é".repeat(12)).is_ok());
    assert!(validate_password(&"a".repeat(1024)).is_ok());
    for invalid in ["a".repeat(11), "a".repeat(1025), "x\0".repeat(12)] {
        assert!(hash_password(&invalid).is_err());
        assert!(!verify_password(LEGACY, &invalid));
    }
}

#[test]
fn matches_existing_observatory_mac_and_rejects_cross_application_reuse() {
    let nonce = "a".repeat(64);
    // Independently computed with Python standard-library HMAC-SHA256.
    let expected = "27d5aee4dcaf639ec84bbb3a2d94f4eba13fafe82f71571f0315b31ce73042bd";
    assert_eq!(
        listener_proof(DOMAIN, ID, ORIGIN, &nonce, SECRET).unwrap(),
        expected
    );
    assert!(verify_listener_proof(DOMAIN, ID, ORIGIN, &nonce, SECRET, expected).is_ok());
    for (domain, id, origin, challenge, secret) in [
        (
            "acp-slack-adapter.local-proof/1",
            ID,
            ORIGIN,
            nonce.as_str(),
            SECRET,
        ),
        (DOMAIN, "other-installation", ORIGIN, nonce.as_str(), SECRET),
        (DOMAIN, ID, "http://127.0.0.1:49153", nonce.as_str(), SECRET),
        (
            DOMAIN,
            ID,
            ORIGIN,
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            SECRET,
        ),
        (
            DOMAIN,
            ID,
            ORIGIN,
            nonce.as_str(),
            "different-fixture-control-token-32-characters",
        ),
    ] {
        assert!(verify_listener_proof(domain, id, origin, challenge, secret, expected).is_err());
    }
}

#[test]
fn proofs_require_bounded_unambiguous_bindings_and_exact_hexadecimal() {
    let nonce = "a".repeat(64);
    for bad in [
        "a".repeat(63),
        "a".repeat(65),
        "A".repeat(64),
        "z".repeat(64),
    ] {
        assert!(listener_proof(DOMAIN, ID, ORIGIN, &bad, SECRET).is_err());
        assert!(verify_listener_proof(DOMAIN, ID, ORIGIN, &nonce, SECRET, &bad).is_err());
    }
    for bad_domain in ["".to_owned(), "x".repeat(129), "domain\nid".into()] {
        assert!(listener_proof(&bad_domain, ID, ORIGIN, &nonce, SECRET).is_err());
    }
    assert!(listener_proof(DOMAIN, "id\nother", ORIGIN, &nonce, SECRET).is_err());
    assert!(listener_proof(DOMAIN, ID, "http://localhost\nother", &nonce, SECRET).is_err());
    assert!(listener_proof(DOMAIN, ID, ORIGIN, &nonce, "too short").is_err());
}
