use super::{AuthError, AuthErrorKind};
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use argon2::{
    Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version,
    password_hash::SaltString,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use rand_core::{OsRng, RngCore};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

pub(super) fn random_token() -> String {
    let mut bytes = Zeroizing::new([0u8; 32]);
    OsRng.fill_bytes(bytes.as_mut());
    URL_SAFE_NO_PAD.encode(bytes.as_ref())
}
pub(super) fn token_valid(token: &str) -> bool {
    token.len() == 43 && URL_SAFE_NO_PAD.decode(token).is_ok_and(|b| b.len() == 32)
}
pub(super) fn hash(domain: &str, value: &str) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(domain.as_bytes());
    h.update([0]);
    h.update(value.as_bytes());
    h.finalize().to_vec()
}
pub(super) fn mac(key: &[u8], domain: &str, value: &str) -> String {
    let mut h = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("HMAC accepts all key lengths");
    h.update(domain.as_bytes());
    h.update(&[0]);
    h.update(value.as_bytes());
    URL_SAFE_NO_PAD.encode(h.finalize().into_bytes())
}
pub(super) fn equal(a: &str, b: &str) -> bool {
    a.len() == b.len() && bool::from(a.as_bytes().ct_eq(b.as_bytes()))
}
pub(super) fn policy_password(password: &str) -> Result<(), AuthError> {
    if !(15..=128).contains(&password.chars().count())
        || password.len() > 512
        || password.chars().any(char::is_control)
    {
        Err(AuthError::new(
            AuthErrorKind::InvalidInput,
            "invalid_input",
            "Use uma senha de 15 a 128 caracteres. Espaços são permitidos.",
        ))
    } else {
        Ok(())
    }
}
pub(super) fn argon() -> Argon2<'static> {
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(65_536, 3, 1, Some(32)).expect("fixed valid password policy"),
    )
}
pub(super) fn password_hash(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    argon()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| AuthError::unavailable())
}
pub(super) fn password_verify(password: &str, encoded: &str) -> Result<(bool, bool), AuthError> {
    let hash = PasswordHash::new(encoded).map_err(|_| AuthError::unavailable())?;
    // Stored PHC parameters are bounded before allocating, including old policies.
    let m = hash.params.get_decimal("m").unwrap_or(0);
    let t = hash.params.get_decimal("t").unwrap_or(0);
    let p = hash.params.get_decimal("p").unwrap_or(0);
    if hash.algorithm.as_str() != "argon2id"
        || !(8..=262_144).contains(&m)
        || !(1..=10).contains(&t)
        || !(1..=4).contains(&p)
    {
        return Err(AuthError::unavailable());
    }
    let valid = argon().verify_password(password.as_bytes(), &hash).is_ok();
    let rehash = m < 65_536
        || t < 3
        || p < 1
        || hash.version != Some(19)
        || hash.hash.is_none_or(|h| h.len() < 32);
    Ok((valid, rehash))
}
pub(super) fn password_rehash(password: &str, encoded: &str) -> Result<String, AuthError> {
    let old = PasswordHash::new(encoded).map_err(|_| AuthError::unavailable())?;
    let params = Params::new(
        old.params.get_decimal("m").unwrap_or(0).max(65_536),
        old.params.get_decimal("t").unwrap_or(0).max(3),
        old.params.get_decimal("p").unwrap_or(0).max(1),
        Some(old.hash.map(|h| h.len()).unwrap_or(0).max(32)),
    )
    .map_err(|_| AuthError::unavailable())?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    argon
        .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
        .map(|h| h.to_string())
        .map_err(|_| AuthError::unavailable())
}
pub(super) fn seal(key: &[u8; 32], aad: &str, plaintext: &[u8]) -> Result<Vec<u8>, AuthError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| AuthError::unavailable())?;
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let encrypted = cipher
        .encrypt(
            &Nonce::from(nonce),
            Payload {
                msg: plaintext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| AuthError::unavailable())?;
    let mut result = Vec::with_capacity(13 + encrypted.len());
    result.push(1);
    result.extend_from_slice(&nonce);
    result.extend_from_slice(&encrypted);
    Ok(result)
}
pub(super) fn open(
    key: &[u8; 32],
    aad: &str,
    ciphertext: &[u8],
) -> Result<Zeroizing<Vec<u8>>, AuthError> {
    if ciphertext.len() < 29 || ciphertext[0] != 1 {
        return Err(AuthError::unavailable());
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| AuthError::unavailable())?;
    let nonce: [u8; 12] = ciphertext[1..13]
        .try_into()
        .map_err(|_| AuthError::unavailable())?;
    cipher
        .decrypt(
            &Nonce::from(nonce),
            Payload {
                msg: &ciphertext[13..],
                aad: aad.as_bytes(),
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| AuthError::unavailable())
}
pub(super) fn mfa_secret() -> Zeroizing<Vec<u8>> {
    let mut bytes = Zeroizing::new(vec![0; 20]);
    OsRng.fill_bytes(&mut bytes);
    bytes
}
pub(super) fn totp(secret: &[u8], step: i64) -> String {
    let mut h = <Hmac<Sha1> as Mac>::new_from_slice(secret).expect("HMAC accepts all key lengths");
    h.update(&(step as u64).to_be_bytes());
    let digest = h.finalize().into_bytes();
    let off = (digest[19] & 15) as usize;
    let n =
        u32::from_be_bytes(digest[off..off + 4].try_into().expect("bounded offset")) & 0x7fff_ffff;
    format!("{:06}", n % 1_000_000)
}
pub(super) fn verify_totp(secret: &[u8], code: &str, now: i64, last: i64) -> Option<i64> {
    let code = code.trim();
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let current = now.div_euclid(30);
    [current - 1, current, current + 1]
        .into_iter()
        .filter(|step| *step >= 0 && *step > last)
        .find(|step| equal(&totp(secret, *step), code))
}
pub(super) fn email(value: &str) -> Result<String, AuthError> {
    let value = value.trim().to_ascii_lowercase();
    let valid = value.is_ascii()
        && value.len() <= 254
        && value.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty()
                && local.len() <= 64
                && !local.starts_with('.')
                && !local.ends_with('.')
                && !local.contains("..")
                && local
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b".!#$%&'*+-/=?^_`{|}~".contains(&b))
                && domain.contains('.')
                && domain.split('.').all(|label| {
                    !label.is_empty()
                        && label.len() <= 63
                        && !label.starts_with('-')
                        && !label.ends_with('-')
                        && label
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                })
        });
    if valid {
        Ok(value)
    } else {
        Err(AuthError::new(
            AuthErrorKind::InvalidInput,
            "invalid_input",
            "Confira o endereço de e-mail informado.",
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rfc6238_vector_and_replay() {
        let s = b"12345678901234567890";
        assert_eq!(totp(s, 59 / 30), "287082");
        assert_eq!(verify_totp(s, "287082", 59, -1), Some(1));
        assert_eq!(verify_totp(s, "287082", 59, 1), None);
    }
    #[test]
    fn encryption_binds_object_and_rejects_tamper() {
        let k = [9; 32];
        let c = seal(&k, "one", b"secret").unwrap();
        assert_eq!(open(&k, "one", &c).unwrap().as_slice(), b"secret");
        assert!(open(&k, "two", &c).is_err());
        let mut t = c;
        t[14] ^= 1;
        assert!(open(&k, "one", &t).is_err());
    }
    #[test]
    fn tokens_have_full_entropy_and_separate_purposes() {
        let a = random_token();
        let b = random_token();
        assert!(token_valid(&a));
        assert_ne!(a, b);
        assert_ne!(hash("reset_password", &a), hash("verify_email", &a));
    }
    #[test]
    fn passwords_preserve_unicode_and_spaces() {
        assert!(policy_password("senha com muitos espaços").is_ok());
        assert!(policy_password("curta").is_err());
        assert!(policy_password(&"a".repeat(129)).is_err());
    }
    #[test]
    fn canonical_email_and_invalid_inputs() {
        assert_eq!(
            email(" Pessoa+obra@EXAMPLE.TEST ").unwrap(),
            "pessoa+obra@example.test"
        );
        for e in [
            "a..b@example.test",
            "a@localhost",
            "a@example..test",
            "a@-example.test",
            "a\n@example.test",
        ] {
            assert!(email(e).is_err());
        }
    }
    #[test]
    fn same_password_has_distinct_salts_and_version() {
        let a = password_hash("frase longa e fictícia").unwrap();
        let b = password_hash("frase longa e fictícia").unwrap();
        assert_ne!(a, b);
        assert!(a.starts_with("$argon2id$v=19$m=65536,t=3,p=1$"));
        assert_eq!(
            password_verify("frase longa e fictícia", &a).unwrap(),
            (true, false)
        );
        assert!(!password_verify("frase errada fictícia", &a).unwrap().0);
    }
    #[test]
    fn rehash_never_reduces_an_existing_cost() {
        let phrase = "longa frase fictícia de teste";
        let weak = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(8192, 1, 1, Some(32)).unwrap(),
        )
        .hash_password(phrase.as_bytes(), &SaltString::generate(&mut OsRng))
        .unwrap()
        .to_string();
        assert_eq!(password_verify(phrase, &weak).unwrap(), (true, true));
        let upgraded = password_rehash(phrase, &weak).unwrap();
        assert_eq!(password_verify(phrase, &upgraded).unwrap(), (true, false));
        let strong = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(65_536, 4, 1, Some(32)).unwrap(),
        )
        .hash_password(phrase.as_bytes(), &SaltString::generate(&mut OsRng))
        .unwrap()
        .to_string();
        assert_eq!(password_verify(phrase, &strong).unwrap(), (true, false));
        let kept = password_rehash(phrase, &strong).unwrap();
        let p = PasswordHash::new(&kept).unwrap();
        assert_eq!(p.params.get_decimal("t"), Some(4));
    }
}
