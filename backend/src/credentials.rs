use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use rand::{rngs::OsRng, RngCore};
use thiserror::Error;

const NONCE_LEN: usize = 12;

#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("credential encryption key must be 32 raw bytes encoded as base64")]
    InvalidKey,
    #[error("stored credential has an invalid format")]
    InvalidCiphertext,
    #[error("credential encryption failed")]
    Encrypt,
    #[error("credential decryption failed")]
    Decrypt,
}

#[derive(Clone)]
pub struct CredentialCipher {
    cipher: Aes256Gcm,
}

impl CredentialCipher {
    pub fn from_base64_key(key: &str) -> Result<Self, CredentialError> {
        let decoded = STANDARD
            .decode(key)
            .map_err(|_| CredentialError::InvalidKey)?;
        let key: [u8; 32] = decoded
            .as_slice()
            .try_into()
            .map_err(|_| CredentialError::InvalidKey)?;

        Ok(Self {
            cipher: Aes256Gcm::new_from_slice(&key).map_err(|_| CredentialError::InvalidKey)?,
        })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String, CredentialError> {
        let mut nonce_bytes = [0_u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|_| CredentialError::Encrypt)?;

        let mut packed = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        packed.extend_from_slice(&nonce_bytes);
        packed.extend_from_slice(&ciphertext);

        Ok(STANDARD.encode(packed))
    }

    pub fn decrypt(&self, ciphertext: &str) -> Result<String, CredentialError> {
        let packed = STANDARD
            .decode(ciphertext)
            .map_err(|_| CredentialError::InvalidCiphertext)?;
        if packed.len() <= NONCE_LEN {
            return Err(CredentialError::InvalidCiphertext);
        }

        let (nonce_bytes, encrypted) = packed.split_at(NONCE_LEN);
        let plaintext = self
            .cipher
            .decrypt(Nonce::from_slice(nonce_bytes), encrypted)
            .map_err(|_| CredentialError::Decrypt)?;

        String::from_utf8(plaintext).map_err(|_| CredentialError::Decrypt)
    }
}
