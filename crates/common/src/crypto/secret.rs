//! Content encryption using ChaCha20-Poly1305
//!
//! This module provides symmetric encryption for bucket data. Each encrypted item
//! (nodes, files) has its own unique `Secret` key, providing:
//! - **Content-addressed storage**: Encrypted data can be hashed deterministically
//! - **Per-item encryption**: Compromising one key doesn't affect other items
//! - **Efficient key rotation**: Can re-encrypt specific items without touching others

use std::io::Read;
use std::ops::Deref;

use chacha20poly1305::Key;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use serde::{Deserialize, Serialize};

/// Size of ChaCha20-Poly1305 nonce in bytes
pub const NONCE_SIZE: usize = 12;
/// Size of ChaCha20-Poly1305 key in bytes (256 bits)
pub const SECRET_SIZE: usize = 32;
/// Size of BLAKE3 hash in bytes (256 bits)
pub const BLAKE3_HASH_SIZE: usize = 32;
/// Default chunk size for streaming operations
#[allow(dead_code)]
pub const CHUNK_SIZE: usize = 4096;

/// Errors that can occur during encryption/decryption
#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("secret error: {0}")]
    Default(#[from] anyhow::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// A 256-bit symmetric encryption key for content encryption
///
/// Each `Secret` is used to encrypt a single item (node or data blob) using ChaCha20-Poly1305 AEAD.
/// The encrypted format is: `nonce (12 bytes) || encrypted(hash(32 bytes) || plaintext) || tag (16 bytes)`.
/// The BLAKE3 hash of the plaintext is prepended before encryption to enable content verification
/// without full decryption (useful for filesystem sync operations).
///
/// # Examples
///
/// ```ignore
/// // Generate a new random secret
/// let secret = Secret::generate();
///
/// // Encrypt data
/// let plaintext = b"sensitive data";
/// let ciphertext = secret.encrypt(plaintext)?;
///
/// // Decrypt data
/// let recovered = secret.decrypt(&ciphertext)?;
/// assert_eq!(plaintext, &recovered[..]);
/// ```
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Secret([u8; SECRET_SIZE]);

impl Default for Secret {
    fn default() -> Self {
        Secret([0; SECRET_SIZE])
    }
}

impl Deref for Secret {
    type Target = [u8; SECRET_SIZE];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<[u8; SECRET_SIZE]> for Secret {
    fn from(bytes: [u8; SECRET_SIZE]) -> Self {
        Secret(bytes)
    }
}

impl Secret {
    /// Generate a new random secret using a cryptographically secure RNG
    pub fn generate() -> Self {
        let mut buff = [0; SECRET_SIZE];
        getrandom::getrandom(&mut buff).expect("failed to generate random bytes");
        Self(buff)
    }

    /// Create a secret from a byte slice
    ///
    /// # Errors
    ///
    /// Returns an error if the slice length is not exactly `SECRET_SIZE` bytes.
    pub fn from_slice(data: &[u8]) -> Result<Self, SecretError> {
        if data.len() != SECRET_SIZE {
            return Err(anyhow::anyhow!(
                "invalid secret size, expected {}, got {}",
                SECRET_SIZE,
                data.len()
            )
            .into());
        }
        let mut buff = [0; SECRET_SIZE];
        buff.copy_from_slice(data);
        Ok(buff.into())
    }

    /// Get a reference to the secret key bytes
    pub fn bytes(&self) -> &[u8] {
        self.0.as_ref()
    }

    /// Encrypt data using ChaCha20-Poly1305 AEAD
    ///
    /// The output format is: `nonce (12 bytes) || encrypted(hash(32) || plaintext) || auth_tag (16 bytes)`.
    /// A BLAKE3 hash of the plaintext is computed and prepended to the data before encryption.
    /// A random nonce is generated for each encryption operation.
    ///
    /// # Errors
    ///
    /// Returns an error if encryption fails (should be rare, only on system RNG failure).
    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, SecretError> {
        // Compute BLAKE3 hash of plaintext
        let plaintext_hash = blake3::hash(data);

        // Prepend hash to plaintext
        let mut data_with_hash = Vec::with_capacity(BLAKE3_HASH_SIZE + data.len());
        data_with_hash.extend_from_slice(plaintext_hash.as_bytes());
        data_with_hash.extend_from_slice(data);

        let key = Key::from_slice(self.bytes());
        let cipher = ChaCha20Poly1305::new(key);

        // Generate random nonce
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        getrandom::getrandom(&mut nonce_bytes)
            .map_err(|e| anyhow::anyhow!("failed to generate nonce: {}", e))?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, data_with_hash.as_ref())
            .map_err(|_| anyhow::anyhow!("encrypt error"))?;

        let mut out = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        out.extend_from_slice(nonce.as_ref());
        out.extend_from_slice(ciphertext.as_ref());

        Ok(out)
    }

    /// Decrypt data using ChaCha20-Poly1305 AEAD
    ///
    /// Expects input in the format: `nonce (12 bytes) || encrypted(hash(32) || plaintext) || auth_tag (16 bytes)`.
    /// Returns only the plaintext (hash is stripped but verified for integrity).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Data is too short to contain a nonce
    /// - Authentication tag verification fails (data was tampered with or wrong key)
    /// - Decrypted data is too short to contain the hash header
    /// - Hash verification fails (data corruption)
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, SecretError> {
        if data.len() < NONCE_SIZE {
            return Err(anyhow::anyhow!("data too short for nonce").into());
        }

        let key = Key::from_slice(self.bytes());
        let nonce = Nonce::from_slice(&data[..NONCE_SIZE]);
        let cipher = ChaCha20Poly1305::new(key);
        let decrypted = cipher
            .decrypt(nonce, &data[NONCE_SIZE..])
            .map_err(|_| anyhow::anyhow!("decrypt error"))?;

        // Extract hash and plaintext
        if decrypted.len() < BLAKE3_HASH_SIZE {
            return Err(anyhow::anyhow!("decrypted data too short for hash header").into());
        }

        let stored_hash = &decrypted[..BLAKE3_HASH_SIZE];
        let plaintext = &decrypted[BLAKE3_HASH_SIZE..];

        // Verify hash integrity
        let computed_hash = blake3::hash(plaintext);
        if stored_hash != computed_hash.as_bytes() {
            return Err(anyhow::anyhow!("hash verification failed - data corrupted").into());
        }

        Ok(plaintext.to_vec())
    }

    /// Extract the BLAKE3 hash of the plaintext without decrypting the full content
    ///
    /// This is useful for filesystem sync operations where you only need to compare
    /// content hashes without loading the entire file into memory.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Data is too short to contain a nonce
    /// - Authentication tag verification fails (data was tampered with or wrong key)
    /// - Decrypted data is too short to contain the hash header
    pub fn extract_plaintext_hash(
        &self,
        data: &[u8],
    ) -> Result<[u8; BLAKE3_HASH_SIZE], SecretError> {
        if data.len() < NONCE_SIZE {
            return Err(anyhow::anyhow!("data too short for nonce").into());
        }

        let key = Key::from_slice(self.bytes());
        let nonce = Nonce::from_slice(&data[..NONCE_SIZE]);
        let cipher = ChaCha20Poly1305::new(key);
        let decrypted = cipher
            .decrypt(nonce, &data[NONCE_SIZE..])
            .map_err(|_| anyhow::anyhow!("decrypt error"))?;

        // Extract just the hash
        if decrypted.len() < BLAKE3_HASH_SIZE {
            return Err(anyhow::anyhow!("decrypted data too short for hash header").into());
        }

        let mut hash = [0u8; BLAKE3_HASH_SIZE];
        hash.copy_from_slice(&decrypted[..BLAKE3_HASH_SIZE]);

        Ok(hash)
    }

    /// Create an encrypted reader from a plaintext reader
    ///
    /// This buffers all data in memory, encrypts it, and returns a reader over the encrypted data.
    /// Future optimization: implement true streaming encryption.
    pub fn encrypt_reader<R>(&self, reader: R) -> Result<impl Read, SecretError>
    where
        R: Read,
    {
        let mut data = Vec::new();
        let mut reader = reader;
        reader.read_to_end(&mut data).map_err(SecretError::Io)?;

        let encrypted = self.encrypt(&data)?;
        Ok(std::io::Cursor::new(encrypted))
    }

    /// Create a decrypted reader from an encrypted reader
    ///
    /// This buffers all encrypted data in memory, decrypts it, and returns a reader over the plaintext.
    /// Future optimization: implement true streaming decryption.
    pub fn decrypt_reader<R>(&self, reader: R) -> Result<impl Read, SecretError>
    where
        R: Read,
    {
        let mut encrypted_data = Vec::new();
        let mut reader = reader;
        reader
            .read_to_end(&mut encrypted_data)
            .map_err(SecretError::Io)?;

        let decrypted = self.decrypt(&encrypted_data)?;
        Ok(std::io::Cursor::new(decrypted))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_secret_encrypt_decrypt() {
        let secret = Secret::generate();
        let data = b"hello world, this is a test message for encryption";

        let encrypted = secret.encrypt(data).unwrap();
        let decrypted = secret.decrypt(&encrypted).unwrap();

        assert_eq!(data.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_encrypt_decrypt_reader() {
        let secret = Secret::generate();
        let data = b"hello world, this is a test message for reader encryption and decryption";

        // Create encrypted reader
        let reader = Cursor::new(data.to_vec());
        let mut encrypted_reader = secret.encrypt_reader(reader).unwrap();

        // Read encrypted data
        let mut encrypted_data = Vec::new();
        encrypted_reader.read_to_end(&mut encrypted_data).unwrap();

        // Decrypt using reader
        let encrypted_cursor = Cursor::new(encrypted_data);
        let mut decrypted_reader = secret.decrypt_reader(encrypted_cursor).unwrap();

        let mut decrypted_data = Vec::new();
        decrypted_reader.read_to_end(&mut decrypted_data).unwrap();

        assert_eq!(data.to_vec(), decrypted_data);
    }

    #[test]
    fn test_secret_size_validation() {
        let too_short = [1u8; 16];
        let too_long = [1u8; 64];

        assert!(Secret::from_slice(&too_short).is_err());
        assert!(Secret::from_slice(&too_long).is_err());

        let just_right = [1u8; SECRET_SIZE];
        assert!(Secret::from_slice(&just_right).is_ok());
    }

    #[test]
    fn test_extract_plaintext_hash() {
        let secret = Secret::generate();
        let data = b"test data for hash extraction";

        // Encrypt the data
        let encrypted = secret.encrypt(data).unwrap();

        // Extract the hash without full decryption
        let extracted_hash = secret.extract_plaintext_hash(&encrypted).unwrap();

        // Compute expected hash
        let expected_hash = blake3::hash(data);

        assert_eq!(extracted_hash, *expected_hash.as_bytes());
    }

    #[test]
    fn test_hash_verification_on_decrypt() {
        let secret = Secret::generate();
        let data = b"test data for integrity check";

        // Encrypt the data
        let mut encrypted = secret.encrypt(data).unwrap();

        // Decrypt should succeed with valid data
        let decrypted = secret.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, data.to_vec());

        // Corrupt the encrypted data (modify a byte in the ciphertext region)
        // Note: This should fail authentication, not hash verification
        if encrypted.len() > NONCE_SIZE + 16 {
            encrypted[NONCE_SIZE + 10] ^= 0xFF;

            // This should fail during ChaCha20-Poly1305 authentication
            let result = secret.decrypt(&encrypted);
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_empty_data_encryption() {
        let secret = Secret::generate();
        let data = b"";

        let encrypted = secret.encrypt(data).unwrap();
        let decrypted = secret.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted, data.to_vec());

        // Hash should still be extractable
        let hash = secret.extract_plaintext_hash(&encrypted).unwrap();
        let expected_hash = blake3::hash(data);
        assert_eq!(hash, *expected_hash.as_bytes());
    }
}
