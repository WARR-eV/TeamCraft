//! Unit tests of the OIDC claim mapping and id_token validation. They live in
//! their own file so the CodeQL configuration can leave test-only code out of
//! the analysis (`.github/codeql/codeql-config.yml`).

use super::*;
use serde_json::json;

fn config(viewer_group: Option<&str>) -> OidcConfig {
    OidcConfig {
        issuer_url: "https://auth.example.org/application/o/teamcraft/".into(),
        client_id: "teamcraft".into(),
        client_secret: "secret".into(),
        scopes: "openid profile email".into(),
        display_name: "SSO".into(),
        groups_claim: "groups".into(),
        username_claim: "preferred_username".into(),
        admin_group: "teamcraft-admins".into(),
        editor_group: "teamcraft-editors".into(),
        viewer_group: viewer_group.map(str::to_string),
    }
}

fn groups(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn group_mapping() {
    let open = config(None);
    assert_eq!(
        role_for_groups(&groups(&["teamcraft-admins", "x"]), &open),
        Some(Role::Admin)
    );
    assert_eq!(
        role_for_groups(&groups(&["TeamCraft-Editors"]), &open),
        Some(Role::Editor)
    );
    assert_eq!(
        role_for_groups(&groups(&["other"]), &open),
        Some(Role::Viewer)
    );
    assert_eq!(role_for_groups(&[], &open), Some(Role::Viewer));

    let restricted = config(Some("teamcraft-users"));
    assert_eq!(role_for_groups(&groups(&["other"]), &restricted), None);
    assert_eq!(
        role_for_groups(&groups(&["teamcraft-users"]), &restricted),
        Some(Role::Viewer)
    );
    assert_eq!(
        role_for_groups(&groups(&["teamcraft-editors"]), &restricted),
        Some(Role::Editor)
    );
}

/// GHSA-864x-fmpf-4p73 (F6): only https, or http on loopback.
#[test]
fn provider_endpoints_need_tls_except_on_loopback() {
    for ok in [
        "https://auth.example.org",
        "https://auth.example.org/application/o/teamcraft/",
        "http://localhost:9000",
        "http://LOCALHOST/realms/x",
        "http://127.0.0.1:8080",
        "http://127.8.9.10",
        "http://[::1]:5556/dex",
    ] {
        assert!(secure_endpoint(ok), "{ok}");
    }
    for bad in [
        "http://auth.example.org",
        "http://192.168.2.10:9000",
        "http://10.0.0.1",
        "http://localhost.example.org",
        "http://[fd00::1]",
        "ftp://auth.example.org",
        "auth.example.org",
        "",
    ] {
        assert!(!secure_endpoint(bad), "{bad}");
    }
}

#[test]
fn id_token_validation() {
    let issuer = "https://auth.example.org/application/o/teamcraft/";
    let good = json!({
        "iss": issuer, "aud": "teamcraft", "exp": 1_000, "nonce": "n1", "sub": "user-1"
    });
    assert!(validate_id_token_claims(&good, issuer, "teamcraft", "n1", 900).is_ok());
    // issuer without trailing slash is equivalent
    assert!(
        validate_id_token_claims(&good, issuer.trim_end_matches('/'), "teamcraft", "n1", 900)
            .is_ok()
    );
    assert!(
        validate_id_token_claims(&good, "https://evil.example/", "teamcraft", "n1", 900).is_err()
    );
    assert!(validate_id_token_claims(&good, issuer, "other-client", "n1", 900).is_err());
    assert!(validate_id_token_claims(&good, issuer, "teamcraft", "wrong", 900).is_err());
    assert!(validate_id_token_claims(&good, issuer, "teamcraft", "n1", 2_000).is_err());
    let multi_aud = json!({
        "iss": issuer, "aud": ["a", "teamcraft"], "exp": 1_000, "nonce": "n1", "sub": "user-1"
    });
    assert!(validate_id_token_claims(&multi_aud, issuer, "teamcraft", "n1", 900).is_ok());
}

#[test]
fn a_fresh_sign_in_needs_a_recent_auth_time() {
    let now = 10_000;
    assert!(fresh_sign_in(&json!({ "auth_time": now - 30 }), now).is_ok());
    assert!(fresh_sign_in(&json!({ "auth_time": now - 300 }), now).is_ok());
    // too old, missing, not a number, far in the future
    assert!(fresh_sign_in(&json!({ "auth_time": now - 301 }), now).is_err());
    assert!(fresh_sign_in(&json!({}), now).is_err());
    assert!(fresh_sign_in(&json!({ "auth_time": "now" }), now).is_err());
    assert!(fresh_sign_in(&json!({ "auth_time": now + 3_600 }), now).is_err());
}

#[test]
fn identity_extraction() {
    let cfg = config(None);
    let claims = json!({
        "sub": "1234",
        "preferred_username": "max.m",
        "name": "Max Mustermann",
        "email": "max@example.org",
        "groups": ["teamcraft-editors", "teamcraft-rocket"]
    });
    let id = identity_from_claims(&claims, &cfg).unwrap();
    assert_eq!(id.subject, "1234");
    assert_eq!(id.username_hint, "max.m");
    assert_eq!(id.display_name, "Max Mustermann");
    assert_eq!(id.email.as_deref(), Some("max@example.org"));
    assert_eq!(id.role, Role::Editor);
    assert_eq!(id.groups, vec!["teamcraft-editors", "teamcraft-rocket"]);

    let restricted = config(Some("teamcraft-users"));
    let outsider = json!({ "sub": "1", "groups": "other" });
    assert!(matches!(
        identity_from_claims(&outsider, &restricted),
        Err(AppError::Forbidden(_))
    ));
}

/// An ES256 key pair as the provider would hold it, and its public JWK.
fn es256_key(kid: &str) -> (ring::signature::EcdsaKeyPair, Jwk) {
    use ring::signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_FIXED_SIGNING};
    let rng = ring::rand::SystemRandom::new();
    let alg = &ECDSA_P256_SHA256_FIXED_SIGNING;
    let pkcs8 = EcdsaKeyPair::generate_pkcs8(alg, &rng).unwrap();
    let pair = EcdsaKeyPair::from_pkcs8(alg, pkcs8.as_ref(), &rng).unwrap();
    let point = pair.public_key().as_ref().to_vec();
    let jwk = Jwk {
        kty: "EC".into(),
        kid: Some(kid.into()),
        alg: Some("ES256".into()),
        crv: Some("P-256".into()),
        x: Some(URL_SAFE_NO_PAD.encode(&point[1..33])),
        y: Some(URL_SAFE_NO_PAD.encode(&point[33..65])),
        ..Jwk::default()
    };
    (pair, jwk)
}

fn sign(pair: &ring::signature::EcdsaKeyPair, header: &Value, claims: &Value) -> String {
    let enc = |v: &Value| URL_SAFE_NO_PAD.encode(serde_json::to_vec(v).unwrap());
    let message = format!("{}.{}", enc(header), enc(claims));
    let rng = ring::rand::SystemRandom::new();
    let sig = pair.sign(&rng, message.as_bytes()).unwrap();
    format!("{message}.{}", URL_SAFE_NO_PAD.encode(sig.as_ref()))
}

const ISS: &str = "https://auth.example.org/application/o/teamcraft/";

fn logout_claims(now: i64) -> Value {
    json!({
        "iss": ISS, "aud": "teamcraft", "iat": now, "jti": "j-1", "sub": "u-4711",
        "events": { BACKCHANNEL_LOGOUT_EVENT: {} },
    })
}

fn check(token: &str, keys: &[Jwk], now: i64) -> Result<String, TokenError> {
    verify_logout_token(token, keys, ISS, "teamcraft", now)
}

#[test]
fn a_signed_logout_token_names_its_subject() {
    let (pair, jwk) = es256_key("k1");
    let now = 1_800_000_000;
    let header = json!({ "alg": "ES256", "kid": "k1", "typ": "logout+jwt" });
    let token = sign(&pair, &header, &logout_claims(now));
    let keys = [jwk];
    assert_eq!(check(&token, &keys, now), Ok("u-4711".to_string()));

    // another key with the same id, an unknown key id
    let (other, mut other_jwk) = es256_key("k1");
    let foreign = sign(&other, &header, &logout_claims(now));
    assert_eq!(check(&foreign, &keys, now), Err(TokenError::BadSignature));
    other_jwk.kid = Some("k2".into());
    assert_eq!(
        check(&token, &[other_jwk], now),
        Err(TokenError::UnknownKey)
    );
    // a changed payload under the old signature, `alg: none`, garbage
    let parts: Vec<&str> = token.split('.').collect();
    let mut claims = logout_claims(now);
    claims["sub"] = json!("admin");
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap());
    let forged = format!("{}.{payload}.{}", parts[0], parts[2]);
    assert_eq!(check(&forged, &keys, now), Err(TokenError::BadSignature));
    let none = URL_SAFE_NO_PAD.encode(br#"{"alg":"none","kid":"k1"}"#);
    let unsigned = format!("{none}.{}.", parts[1]);
    assert!(check(&unsigned, &keys, now).is_err());
    assert_eq!(check("a.b", &keys, now), Err(TokenError::Malformed));
}

#[test]
fn logout_token_claims_are_checked() {
    let (pair, jwk) = es256_key("k1");
    let now = 1_800_000_000;
    let header = json!({ "alg": "ES256", "kid": "k1" });
    let keys = [jwk];
    let refused = |change: &dyn Fn(&mut Value)| {
        let mut claims = logout_claims(now);
        change(&mut claims);
        let token = sign(&pair, &header, &claims);
        matches!(check(&token, &keys, now), Err(TokenError::Claim(_)))
    };
    assert!(refused(&|c| c["iss"] = json!("https://evil.example.org")));
    assert!(refused(&|c| c["aud"] = json!("another-app")));
    assert!(refused(&|c| c["iat"] = json!(now - 3600)));
    assert!(refused(&|c| c["iat"] = json!(now + 3600)));
    assert!(refused(&|c| c["exp"] = json!(now - 3600)));
    assert!(refused(&|c| c["events"] = json!({})));
    assert!(refused(&|c| c["nonce"] = json!("n")));
    assert!(refused(&|c| c["sub"] = json!("")));
    // a list audience is fine
    let mut claims = logout_claims(now);
    claims["aud"] = json!(["other", "teamcraft"]);
    let token = sign(&pair, &header, &claims);
    assert!(check(&token, &keys, now).is_ok());
}

/// A compact JWS of `claims` with an HMAC of `secret`.
fn sign_hmac(alg: &str, secret: &[u8], header: &Value, claims: &Value) -> String {
    let enc = |v: &Value| URL_SAFE_NO_PAD.encode(serde_json::to_vec(v).unwrap());
    let message = format!("{}.{}", enc(header), enc(claims));
    let key = ring::hmac::Key::new(hmac_algorithm(alg).unwrap(), secret);
    let tag = ring::hmac::sign(&key, message.as_bytes());
    format!("{message}.{}", URL_SAFE_NO_PAD.encode(tag.as_ref()))
}

/// GHSA-864x-fmpf-4p73 (F6): the ID token's signature is checked.
#[test]
fn id_token_signatures_are_checked() {
    let claims = json!({ "iss": ISS, "aud": "teamcraft", "sub": "u-4711" });
    let (pair, jwk) = es256_key("k1");
    let keys = [jwk.clone()];
    let header = json!({ "alg": "ES256", "kid": "k1", "typ": "JWT" });
    let token = sign(&pair, &header, &claims);
    assert_eq!(
        verified_payload(&token, &keys, None).unwrap()["sub"],
        "u-4711"
    );
    // without a kid every key of the right type is tried
    let token = sign(&pair, &json!({ "alg": "ES256" }), &claims);
    assert!(verified_payload(&token, &keys, None).is_ok());

    // a changed payload under the old signature, a foreign key, `none`
    let parts: Vec<&str> = token.split('.').collect();
    let admin = URL_SAFE_NO_PAD.encode(br#"{"iss":"x","aud":"teamcraft","sub":"admin"}"#);
    let forged = format!("{}.{admin}.{}", parts[0], parts[2]);
    assert_eq!(
        verified_payload(&forged, &keys, None),
        Err(TokenError::BadSignature)
    );
    let (other, _) = es256_key("k1");
    let foreign = sign(&other, &header, &claims);
    assert_eq!(
        verified_payload(&foreign, &keys, None),
        Err(TokenError::BadSignature)
    );
    let none = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
    assert!(verified_payload(&format!("{none}.{}.", parts[1]), &keys, None).is_err());
    assert_eq!(
        verified_payload(&token, &[], None),
        Err(TokenError::UnknownKey)
    );

    // an encryption key never signs; a key pinned to another algorithm neither
    let mut enc = jwk.clone();
    enc.use_ = Some("enc".into());
    assert_eq!(
        verified_payload(&token, &[enc], None),
        Err(TokenError::BadSignature)
    );
    let mut pinned = jwk.clone();
    pinned.alg = Some("RS256".into());
    assert_eq!(
        verified_payload(&token, &[pinned], None),
        Err(TokenError::BadSignature)
    );
    // ES256 signed, but the header claims ES384: refused
    let lie = sign(&pair, &json!({ "alg": "ES384", "kid": "k1" }), &claims);
    let mut es = jwk;
    es.alg = None;
    assert_eq!(
        verified_payload(&lie, &[es], None),
        Err(TokenError::BadSignature)
    );
    // unknown critical header extensions
    let crit = sign(
        &pair,
        &json!({ "alg": "ES256", "kid": "k1", "crit": ["exp"] }),
        &claims,
    );
    assert_eq!(
        verified_payload(&crit, &keys, None),
        Err(TokenError::Malformed)
    );
}

#[test]
fn es384_and_hmac_id_tokens() {
    use ring::signature::{EcdsaKeyPair, KeyPair, ECDSA_P384_SHA384_FIXED_SIGNING};
    let claims = json!({ "iss": ISS, "aud": "teamcraft", "sub": "u-4711" });
    let rng = ring::rand::SystemRandom::new();
    let alg = &ECDSA_P384_SHA384_FIXED_SIGNING;
    let pkcs8 = EcdsaKeyPair::generate_pkcs8(alg, &rng).unwrap();
    let pair = EcdsaKeyPair::from_pkcs8(alg, pkcs8.as_ref(), &rng).unwrap();
    let point = pair.public_key().as_ref().to_vec();
    let jwk = Jwk {
        kty: "EC".into(),
        kid: Some("p384".into()),
        crv: Some("P-384".into()),
        x: Some(URL_SAFE_NO_PAD.encode(&point[1..49])),
        y: Some(URL_SAFE_NO_PAD.encode(&point[49..97])),
        ..Jwk::default()
    };
    let token = sign(&pair, &json!({ "alg": "ES384", "kid": "p384" }), &claims);
    assert!(verified_payload(&token, &[jwk], None).is_ok());

    // HS256 with the client secret (OIDC Core 10.1); no secret, a wrong one
    let secret = b"a-long-client-secret-of-the-test";
    for alg in ["HS256", "HS384", "HS512"] {
        let token = sign_hmac(alg, secret, &json!({ "alg": alg }), &claims);
        assert!(verified_payload(&token, &[], Some(secret)).is_ok(), "{alg}");
        assert_eq!(
            verified_payload(&token, &[], Some(b"another-secret")),
            Err(TokenError::BadSignature)
        );
        assert_eq!(
            verified_payload(&token, &[], None),
            Err(TokenError::BadSignature)
        );
        assert_eq!(
            verified_payload(&token, &[], Some(b"")),
            Err(TokenError::BadSignature)
        );
        assert_eq!(token_alg(&token), alg);
    }
    // a logout token signed with the client secret stays refused
    let logout = sign_hmac(
        "HS256",
        secret,
        &json!({ "alg": "HS256" }),
        &logout_claims(1_800_000_000),
    );
    assert!(check(&logout, &[], 1_800_000_000).is_err());
}
