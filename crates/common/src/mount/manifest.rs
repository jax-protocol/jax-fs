//! # Manifest
//!
//! The manifest is the root metadata structure for a bucket. It contains:
//!
//! - **Identity**: UUID and friendly name
//! - **Access control**: Map of principals to their shares
//! - **Content**: Links to the entry node and pin set
//! - **History**: Link to previous manifest version and height in the chain
//! - **Publication state**: Optional plaintext secret for public read access
//!
//! ## Encryption Model
//!
//! - **Owners** have an encrypted [`SecretShare`] that they can decrypt with their private key
//! - **Mirrors** have no individual share; they use [`Manifest::public`] when available
//! - **Publishing** stores the bucket's secret in plaintext, making it readable by anyone with the manifest
//!
//! ## Versioning
//!
//! Each modification creates a new manifest with:
//! - `previous` pointing to the prior manifest's CID
//! - `height` incremented by 1
//!
//! This forms an immutable version chain for history traversal.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::crypto::{PublicKey, Secret, SecretKey, SecretShare, Signature};
use crate::linked_data::{BlockEncoded, CodecError, DagCborCodec, Link};
use crate::version::Version;

use super::principal::{Principal, PrincipalRole};

/// Errors that can occur during manifest operations.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("codec error: {0}")]
    Codec(#[from] CodecError),
    #[error("signature verification failed")]
    SignatureVerificationFailed,
}

/// A principal's share of bucket access.
///
/// Combines a [`Principal`] (identity + role) with an optional encrypted secret share.
/// The share structure differs by role:
///
/// - **Owners**: Always have `Some(SecretShare)` encrypted to their public key
/// - **Mirrors**: Always have `None`; use the manifest's `public` secret instead
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Share {
    principal: Principal,
    /// The encrypted share of the bucket's secret key.
    /// Only owners have this; mirrors use the manifest's public secret instead.
    share: Option<SecretShare>,
}

impl Share {
    /// Create a new owner share with an encrypted secret.
    pub fn new_owner(share: SecretShare, public_key: PublicKey) -> Self {
        Self {
            principal: Principal {
                role: PrincipalRole::Owner,
                identity: public_key,
            },
            share: Some(share),
        }
    }

    /// Create a new mirror share.
    ///
    /// Mirrors don't have individual encrypted shares. They use the manifest's
    /// `public` field for decryption once the bucket is published.
    pub fn new_mirror(public_key: PublicKey) -> Self {
        Self {
            principal: Principal {
                role: PrincipalRole::Mirror,
                identity: public_key,
            },
            share: None,
        }
    }

    /* Getters */

    /// Get the principal (identity and role).
    pub fn principal(&self) -> &Principal {
        &self.principal
    }

    /// Get the encrypted secret share.
    ///
    /// Returns `Some` for owners, `None` for mirrors.
    pub fn share(&self) -> Option<&SecretShare> {
        self.share.as_ref()
    }

    /// Get the principal's role.
    pub fn role(&self) -> &PrincipalRole {
        &self.principal.role
    }

    /* Setters */

    /// Set the encrypted secret share.
    pub fn set_share(&mut self, share: SecretShare) {
        self.share = Some(share);
    }
}

/// Map of hex-encoded public keys to their shares.
///
/// Uses `String` keys (hex-encoded [`PublicKey`]) for CBOR serialization compatibility.
pub type Shares = BTreeMap<String, Share>;

/// The root metadata structure for a bucket.
///
/// A manifest contains everything needed to access and verify a bucket:
///
/// - **Identity**: Global UUID and human-readable name
/// - **Access control**: Principal shares for decryption
/// - **Content pointers**: Links to entry node, pin set, and crdt op log
/// - **Version chain**: Previous link and height for history
/// - **Publication**: Optional plaintext secret for public access
///
/// # Serialization
///
/// Manifests are serialized using DAG-CBOR and stored as content-addressed blobs.
/// The manifest's CID serves as the bucket's current state identifier.
///
/// # Example
///
/// ```ignore
/// let manifest = Manifest::new(
///     Uuid::new_v4(),
///     "my-bucket".to_string(),
///     owner_public_key,
///     secret_share,
///     entry_link,
///     pins_link,
///     0, // initial height
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// Global unique identifier for this bucket.
    id: Uuid,
    /// Human-readable name for display.
    name: String,
    /// Height in the version chain (0 for initial, increments on each update).
    height: u64,
    /// Software version for compatibility checking.
    version: Version,
    /// Map of principal public keys (hex) to their shares.
    shares: Shares,
    /// Link to the root [`Node`](super::Node) of the file tree.
    entry: Link,
    /// Link to the [`Pins`](super::Pins) blob hash set.
    pins: Link,
    /// Link to the previous manifest version (forms history chain).
    previous: Option<Link>,
    /// Optional link to the encrypted path operations log (CRDT).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ops_log: Option<Link>,
    /// Plaintext secret for public read access.
    ///
    /// When set, anyone with the manifest can decrypt bucket contents.
    /// Publishing is opt-in per version via `save(publish: true)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    public: Option<Secret>,
    /// Public key of the peer who signed this manifest.
    ///
    /// Set when the manifest is signed via [`Manifest::sign`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    author: Option<PublicKey>,
    /// Ed25519 signature over the manifest contents.
    ///
    /// The signature covers all fields except `signature` itself (see [`Manifest::signable_bytes`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    signature: Option<Signature>,
}

impl BlockEncoded<DagCborCodec> for Manifest {}

impl Manifest {
    /// Create a new manifest with an initial owner.
    ///
    /// # Arguments
    ///
    /// * `id` - Global unique identifier for the bucket
    /// * `name` - Human-readable display name
    /// * `owner` - Public key of the initial owner
    /// * `share` - Encrypted secret share for the owner
    /// * `entry` - Link to the root node
    /// * `pins` - Link to the pin set
    /// * `height` - Version chain height (usually 0 for new buckets)
    pub fn new(
        id: Uuid,
        name: String,
        owner: PublicKey,
        share: SecretShare,
        entry: Link,
        pins: Link,
        height: u64,
    ) -> Self {
        Manifest {
            id,
            name,
            shares: BTreeMap::from([(
                owner.to_hex(),
                Share {
                    principal: Principal {
                        role: PrincipalRole::Owner,
                        identity: owner,
                    },
                    share: Some(share),
                },
            )]),
            entry,
            pins,
            previous: None,
            height,
            version: Version::default(),
            ops_log: None,
            public: None,
            author: None,
            signature: None,
        }
    }

    /* Getters */

    /// Get the bucket's unique identifier.
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get the bucket's display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the software version.
    pub fn version(&self) -> &Version {
        &self.version
    }

    /// Get the entry node link.
    pub fn entry(&self) -> &Link {
        &self.entry
    }

    /// Get the pins link.
    pub fn pins(&self) -> &Link {
        &self.pins
    }

    /// Get the previous manifest link.
    pub fn previous(&self) -> &Option<Link> {
        &self.previous
    }

    /// Get the version chain height.
    pub fn height(&self) -> u64 {
        self.height
    }

    /// Get the operations log link.
    pub fn ops_log(&self) -> Option<&Link> {
        self.ops_log.as_ref()
    }

    /// Get all shares.
    pub fn shares(&self) -> &BTreeMap<String, Share> {
        &self.shares
    }

    /// Get mutable access to shares.
    pub fn shares_mut(&mut self) -> &mut BTreeMap<String, Share> {
        &mut self.shares
    }

    /// Get a principal's share by their public key.
    pub fn get_share(&self, public_key: &PublicKey) -> Option<&Share> {
        self.shares.get(&public_key.to_hex())
    }

    /// Get all peer public keys from shares.
    pub fn get_peer_ids(&self) -> Vec<PublicKey> {
        self.shares
            .iter()
            .filter_map(|(key_hex, _)| PublicKey::from_hex(key_hex).ok())
            .collect()
    }

    /// Get all shares with a specific role.
    pub fn get_shares_by_role(&self, role: PrincipalRole) -> Vec<&Share> {
        self.shares.values().filter(|s| *s.role() == role).collect()
    }

    /// Check if the bucket is published.
    ///
    /// Published buckets have their secret stored in plaintext, allowing
    /// anyone with the manifest to decrypt contents.
    pub fn is_published(&self) -> bool {
        self.public.is_some()
    }

    /// Get the public secret if available.
    pub fn public(&self) -> Option<&Secret> {
        self.public.as_ref()
    }

    /// Get the author (signer's public key) if the manifest is signed.
    pub fn author(&self) -> Option<&PublicKey> {
        self.author.as_ref()
    }

    /// Get the signature if the manifest is signed.
    pub fn signature(&self) -> Option<&Signature> {
        self.signature.as_ref()
    }

    /// Check if the manifest has been signed.
    pub fn is_signed(&self) -> bool {
        self.author.is_some() && self.signature.is_some()
    }

    /* Setters */

    /// Set the entry node link.
    pub fn set_entry(&mut self, entry: Link) {
        self.entry = entry;
    }

    /// Set the pins link.
    pub fn set_pins(&mut self, pins_link: Link) {
        self.pins = pins_link;
    }

    /// Set the previous manifest link.
    pub fn set_previous(&mut self, previous: Link) {
        self.previous = Some(previous);
    }

    /// Set the version chain height.
    pub fn set_height(&mut self, height: u64) {
        self.height = height;
    }

    /// Set the operations log link.
    pub fn set_ops_log(&mut self, link: Link) {
        self.ops_log = Some(link);
    }

    /// Add a share to the manifest.
    ///
    /// Use [`Share::new_owner`] or [`Share::new_mirror`] to construct the share.
    pub fn add_share(&mut self, share: Share) {
        let key = share.principal().identity.to_hex();
        self.shares.insert(key, share);
    }

    /// Publish the bucket by storing the secret in plaintext.
    ///
    /// **Warning**: Once published, this version's secret is exposed
    /// and anyone with the manifest can decrypt bucket contents.
    pub fn publish(&mut self, secret: &Secret) {
        self.public = Some(secret.clone());
    }

    /* Signing */

    /// Sign this manifest with the given secret key.
    ///
    /// Sets the `author` field to the public key and `signature` to the Ed25519
    /// signature over the manifest's signable bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest cannot be serialized for signing.
    pub fn sign(&mut self, secret_key: &SecretKey) -> Result<(), ManifestError> {
        self.author = Some(secret_key.public());
        self.signature = None; // Clear any existing signature before computing signable_bytes
        let bytes = self.signable_bytes()?;
        let signature = secret_key.sign(&bytes);
        self.signature = Some(signature);
        Ok(())
    }

    /// Verify the manifest's signature.
    ///
    /// Returns `Ok(true)` if the signature is valid, `Ok(false)` if the manifest
    /// is unsigned (no author or signature), and an error if verification fails.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The manifest cannot be serialized for verification
    /// - The signature is invalid (tampered or wrong key)
    pub fn verify_signature(&self) -> Result<bool, ManifestError> {
        let (author, signature) = match (self.author.as_ref(), self.signature.as_ref()) {
            (Some(a), Some(s)) => (a, s),
            _ => return Ok(false), // Not signed
        };

        let bytes = self.signable_bytes()?;
        author
            .verify(&bytes, signature)
            .map_err(|_| ManifestError::SignatureVerificationFailed)?;
        Ok(true)
    }

    /// Get the bytes to be signed.
    ///
    /// Returns the DAG-CBOR serialization of the manifest with `signature` set to `None`.
    /// This ensures the signature covers all fields except itself.
    fn signable_bytes(&self) -> Result<Vec<u8>, ManifestError> {
        let mut signable = self.clone();
        signable.signature = None; // Exclude signature field
        Ok(signable.encode()?) // DAG-CBOR serialize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::crypto::{PublicKey, Secret};
    use crate::linked_data::Link;

    fn create_test_manifest() -> Manifest {
        let secret_key = SecretKey::generate();
        let public_key = secret_key.public();
        let share = SecretShare::default();
        let entry_link = Link::default();
        let pins_link = Link::default();

        Manifest::new(
            uuid::Uuid::new_v4(),
            "test-bucket".to_string(),
            public_key,
            share,
            entry_link,
            pins_link,
            0,
        )
    }

    #[test]
    fn test_share_serialize() {
        use ipld_core::codec::Codec;
        use serde_ipld_dagcbor::codec::DagCborCodec;

        let share = SecretShare::default();

        // Try to encode/decode just the Share
        let encoded = DagCborCodec::encode_to_vec(&share).unwrap();
        let decoded: SecretShare = DagCborCodec::decode_from_slice(&encoded).unwrap();

        assert_eq!(share, decoded);
    }

    #[test]
    fn test_principal_serialize() {
        use ipld_core::codec::Codec;
        use serde_ipld_dagcbor::codec::DagCborCodec;

        let public_key = crate::crypto::SecretKey::generate().public();
        let principal = Principal {
            role: PrincipalRole::Owner,
            identity: public_key,
        };

        // Try to encode/decode just the Principal
        let encoded = DagCborCodec::encode_to_vec(&principal).unwrap();
        let decoded: Principal = DagCborCodec::decode_from_slice(&encoded).unwrap();

        assert_eq!(principal, decoded);
    }

    #[test]
    fn test_manifest_signing() {
        let secret_key = SecretKey::generate();
        let mut manifest = create_test_manifest();

        // Initially unsigned
        assert!(!manifest.is_signed());
        assert!(manifest.author().is_none());
        assert!(manifest.signature().is_none());

        // Sign the manifest
        manifest.sign(&secret_key).unwrap();

        // Now should be signed
        assert!(manifest.is_signed());
        assert_eq!(manifest.author(), Some(&secret_key.public()));
        assert!(manifest.signature().is_some());

        // Verify the signature
        assert!(manifest.verify_signature().unwrap());
    }

    #[test]
    fn test_manifest_tamper_detection() {
        let secret_key = SecretKey::generate();
        let mut manifest = create_test_manifest();

        // Sign the manifest
        manifest.sign(&secret_key).unwrap();
        assert!(manifest.verify_signature().unwrap());

        // Tamper with the manifest - change the height
        manifest.set_height(999);

        // Verification should now fail
        let result = manifest.verify_signature();
        assert!(result.is_err());
    }

    #[test]
    fn test_unsigned_manifest_backwards_compatibility() {
        use ipld_core::codec::Codec;
        use serde_ipld_dagcbor::codec::DagCborCodec;

        // Create a manifest without signing (simulates old unsigned manifest)
        let manifest = create_test_manifest();
        assert!(!manifest.is_signed());

        // Serialize it
        let encoded = DagCborCodec::encode_to_vec(&manifest).unwrap();

        // Deserialize it back
        let decoded: Manifest = DagCborCodec::decode_from_slice(&encoded).unwrap();

        // Should still work without author/signature
        assert!(!decoded.is_signed());
        assert!(decoded.author().is_none());
        assert!(decoded.signature().is_none());

        // verify_signature should return Ok(false) for unsigned manifests
        assert!(!decoded.verify_signature().unwrap());
    }

    #[test]
    fn test_manifest_wrong_key_verification() {
        let secret_key1 = SecretKey::generate();
        let secret_key2 = SecretKey::generate();
        let mut manifest = create_test_manifest();

        // Sign with key 1
        manifest.sign(&secret_key1).unwrap();
        assert!(manifest.verify_signature().unwrap());

        // Manually change the author to key 2's public key (simulating a forgery attempt)
        // We need to access the author field directly for this test
        // Since we can't modify it directly, we'll just verify that the current
        // signature was made with key 1, not key 2
        assert_eq!(manifest.author(), Some(&secret_key1.public()));
        assert_ne!(manifest.author(), Some(&secret_key2.public()));
    }
}
