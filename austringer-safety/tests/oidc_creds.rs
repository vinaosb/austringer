//! OIDC workload identity + credential hygiene tests (todo 26).

#![cfg(test)]

use austringer_safety::creds::{CredentialError, CredentialHelper, InMemoryCredentialHelper};
use austringer_safety::oidc::{
    HmacSigner, MockSigner, OidcSigner, SubjectTemplateClaims, TokenRequest,
    WorkloadIdentityService, verify_jwt,
};

fn template() -> SubjectTemplateClaims {
    SubjectTemplateClaims {
        principal: "agent@austringer".into(),
        email: Some("agent@example.com".into()),
        teams: vec!["eng".into(), "sandbox".into()],
        environment: "test".into(),
        agent_name: "worker-1".into(),
        run_id: "run-abc".into(),
    }
}

#[test]
fn hmac_signer_matches_rfc4231_vector_1() {
    // RFC 4231 test case 1: key = 0x0b x20, data = "Hi There"
    let signer = HmacSigner::with_key(vec![0x0b; 20]);
    let mac = signer.sign("Hi There");
    let expected: [u8; 32] = [
        0xb0, 0x34, 0x4c, 0x61, 0xd8, 0xdb, 0x38, 0x53, 0x5c, 0xa8, 0xaf, 0xce, 0xaf, 0x0b, 0xf1,
        0x2b, 0x88, 0x1d, 0xc2, 0x00, 0xc9, 0x83, 0x3d, 0xa7, 0x26, 0xe9, 0x37, 0x6c, 0x2e, 0x32,
        0xcf, 0xf7,
    ];
    assert_eq!(mac, expected.to_vec(), "signer must be real HMAC-SHA256");
}

#[test]
fn minted_jwt_carries_all_federation_claims() {
    let signer = HmacSigner::with_key(vec![7; 32]);
    let service = WorkloadIdentityService::new(template(), signer, || 1_000);
    let response = service
        .mint_for(&TokenRequest {
            audience: "cloud-api".into(),
            ttl_secs: 60,
        })
        .expect("mint");
    let claims = verify_jwt(&response.jwt, &service.signer, 1_000).expect("verify");
    assert_eq!(
        claims.get("principal").and_then(|v| v.as_str()),
        Some("agent@austringer")
    );
    assert_eq!(
        claims.get("email").and_then(|v| v.as_str()),
        Some("agent@example.com")
    );
    assert_eq!(
        claims.get("environment").and_then(|v| v.as_str()),
        Some("test")
    );
    assert_eq!(
        claims.get("agent_name").and_then(|v| v.as_str()),
        Some("worker-1")
    );
    assert_eq!(
        claims.get("run_id").and_then(|v| v.as_str()),
        Some("run-abc")
    );
    assert_eq!(
        claims.get("aud").and_then(|v| v.as_str()),
        Some("cloud-api")
    );
    assert_eq!(
        claims
            .get("teams")
            .and_then(|v| v.as_array())
            .map(|a| a.len()),
        Some(2)
    );
}

#[test]
fn expired_jwt_is_typed_error() {
    let signer = HmacSigner::with_key(vec![7; 32]);
    let service = WorkloadIdentityService::new(template(), signer, || 1_000);
    let response = service
        .mint_for(&TokenRequest {
            audience: "cloud-api".into(),
            ttl_secs: 60,
        })
        .expect("mint");
    let later = 1_000 + 61;
    match verify_jwt(&response.jwt, &service.signer, later) {
        Err(austringer_safety::oidc::TokenError::Expired) => {}
        other => panic!("expected Expired, got {other:?}"),
    }
}

#[test]
fn tampered_jwt_signature_is_rejected() {
    let signer = HmacSigner::with_key(vec![7; 32]);
    let service = WorkloadIdentityService::new(template(), signer, || 1_000);
    let response = service
        .mint_for(&TokenRequest {
            audience: "cloud-api".into(),
            ttl_secs: 60,
        })
        .expect("mint");
    let mut token = response.jwt.clone();
    // flip a character inside the signature segment (last segment)
    let last = token.rfind('.').expect("two dots");
    let sig_start = last + 1;
    token.replace_range(
        sig_start..sig_start + 1,
        if token.as_bytes()[sig_start] == b'A' {
            "B"
        } else {
            "A"
        },
    );
    assert!(verify_jwt(&token, &service.signer, 1_000).is_err());
}

#[test]
fn mock_signer_round_trip_for_offline_tests() {
    let mock = MockSigner::new(b"mock-key".to_vec());
    let service = WorkloadIdentityService::new(template(), mock, || 5_000);
    let response = service
        .mint_for(&TokenRequest {
            audience: "a".into(),
            ttl_secs: 30,
        })
        .expect("mint");
    assert!(verify_jwt(&response.jwt, &service.signer, 5_000).is_ok());
}

#[test]
fn credential_helper_mints_short_lived_per_command_tokens() {
    let mut helper = InMemoryCredentialHelper::with_clock(|| 10_000);
    let token = helper.mint_for_command("cargo build", 60).expect("mint");
    let fetched = helper.credential_for("cargo build").expect("credential");
    assert_eq!(fetched.value, token.value);
    assert!(fetched.is_valid(10_030));
    assert!(!fetched.is_valid(10_061), "token must expire after ttl");
}

#[test]
fn expired_credential_is_typed_error_never_stale_value() {
    static CLOCK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    fn clock() -> u64 {
        CLOCK.load(std::sync::atomic::Ordering::SeqCst)
    }
    CLOCK.store(100, std::sync::atomic::Ordering::SeqCst);
    let mut helper = InMemoryCredentialHelper::with_clock(clock);
    helper.mint_for_command("cargo test", 10).expect("mint"); // expires 110
    assert!(helper.credential_for("cargo test").is_ok());
    CLOCK.store(111, std::sync::atomic::Ordering::SeqCst);
    match helper.credential_for("cargo test") {
        Err(CredentialError::Expired) => {}
        other => panic!("expected Expired, got {other:?}"),
    }
}

#[test]
fn unknown_command_has_no_credential_fail_closed() {
    let helper = InMemoryCredentialHelper::new();
    assert!(matches!(
        helper.credential_for("anything"),
        Err(CredentialError::NoCredential)
    ));
}

#[test]
fn short_lived_token_debug_never_leaks_value() {
    let mut helper = InMemoryCredentialHelper::with_clock(|| 10_000);
    let token = helper.mint_for_command("cargo build", 60).expect("mint");
    let rendered = format!("{token:?}");
    assert!(
        !rendered.contains(&token.value),
        "Debug must redact the secret"
    );
}
