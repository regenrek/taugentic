use base64::Engine;
use rand::RngCore;
use sha2::Digest;
use sha2::Sha256;

pub const PKCE_METHOD_S256: &str = "S256";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PkcePair {
    pub verifier: String,
    pub challenge: String,
    pub method: &'static str,
}

impl PkcePair {
    pub fn generate() -> Self {
        let mut bytes = [0_u8; 64];
        rand::rng().fill_bytes(&mut bytes);
        let verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        let challenge = challenge_for_verifier(&verifier);

        Self {
            verifier,
            challenge,
            method: PKCE_METHOD_S256,
        }
    }
}

pub fn challenge_for_verifier(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

#[cfg(test)]
mod tests {
    use super::{PKCE_METHOD_S256, PkcePair, challenge_for_verifier};

    #[test]
    fn generated_pair_round_trips_challenge() {
        let pair = PkcePair::generate();

        assert_eq!(pair.method, PKCE_METHOD_S256);
        assert_eq!(pair.challenge, challenge_for_verifier(&pair.verifier));
    }

    #[test]
    fn verifier_length_is_within_pkce_bounds() {
        let pair = PkcePair::generate();

        assert!((43..=128).contains(&pair.verifier.len()));
    }

    #[test]
    fn verifier_uses_url_safe_base64_without_padding() {
        let pair = PkcePair::generate();

        assert!(
            pair.verifier
                .bytes()
                .all(|byte| { byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_') })
        );
        assert!(!pair.verifier.contains('='));
    }

    #[test]
    fn challenge_uses_rfc7636_test_vector() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";

        assert_eq!(
            challenge_for_verifier(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
}
