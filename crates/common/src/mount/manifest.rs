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
//! - **Mirrors** have no individual share; they use [`Manifest::published_secret`] when available
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

use crate::crypto::{PublicKey, Secret, SecretShare};
use crate::linked_data::{BlockEncoded, DagCborCodec, Link};
use crate::version::Version;

use super::principal::{Principal, PrincipalRole};

/// A principal's share of bucket access.
///
/// Combines a [`Principal`] (identity + role) with an optional encrypted secret share.
/// The share structure differs by role:
///
/// - **Owners**: Always have `Some(SecretShare)` encrypted to their public key
/// - **Mirrors**: Always have `None`; use the manifest's `published_secret` instead
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Share {
    principal: Principal,
    /// The encrypted share of the bucket's secret key.
    /// Only owners have this; mirrors use the manifest's published_secret instead.
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
    /// `published_secret` field for decryption once the bucket is published.
    pub fn new_mirror(public_key: PublicKey) -> Self {
        Self {
            principal: Principal {
                role: PrincipalRole::Mirror,
                identity: public_key,
            },
            share: None,
        }
    }

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
    /// Map of principal public keys (hex) to their shares.
    shares: Shares,
    /// Link to the root [`Node`](super::Node) of the file tree.
    entry: Link,
    /// Link to the [`Pins`](super::Pins) blob hash set.
    pins: Link,
    /// Link to the previous manifest version (forms history chain).
    previous: Option<Link>,
    /// Height in the version chain (0 for initial, increments on each update).
    height: u64,
    /// Software version for compatibility checking.
    version: Version,
    /// Optional link to the encrypted path operations log (CRDT).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ops_log: Option<Link>,
    /// Plaintext secret for public read access.
    ///
    /// When set, anyone with the manifest can decrypt bucket contents.
    /// Once published, this cannot be revoked - the secret is exposed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    published_secret: Option<Secret>,
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
            published_secret: None,
        }
    }

    /// Get a principal's share by their public key.
    pub fn get_share(&self, public_key: &PublicKey) -> Option<&Share> {
        self.shares.get(&public_key.to_hex())
    }

    /// Add a share to the manifest.
    ///
    /// Use [`Share::new_owner`] or [`Share::new_mirror`] to construct the share.
    pub fn add_share(&mut self, share: Share) {
        let key = share.principal().identity.to_hex();
        self.shares.insert(key, share);
    }

    /// Remove all shares from the manifest.
    pub fn unset_shares(&mut self) {
        self.shares.clear();
    }

    /// Get the bucket's unique identifier.
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get the bucket's display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get all shares.
    pub fn shares(&self) -> &BTreeMap<String, Share> {
        &self.shares
    }

    /// Get the software version.
    pub fn version(&self) -> &Version {
        &self.version
    }

    /// Get the entry node link.
    pub fn entry(&self) -> &Link {
        &self.entry
    }

    /// Set the entry node link.
    pub fn set_entry(&mut self, entry: Link) {
        self.entry = entry;
    }

    /// Get the pins link.
    pub fn pins(&self) -> &Link {
        &self.pins
    }

    /// Set the pins link.
    pub fn set_pins(&mut self, pins_link: Link) {
        self.pins = pins_link;
    }

    /// Set the previous manifest link.
    pub fn set_previous(&mut self, previous: Link) {
        self.previous = Some(previous);
    }

    /// Get the previous manifest link.
    pub fn previous(&self) -> &Option<Link> {
        &self.previous
    }

    /// Get the version chain height.
    pub fn height(&self) -> u64 {
        self.height
    }

    /// Set the version chain height.
    pub fn set_height(&mut self, height: u64) {
        self.height = height;
    }

    /// Get all peer public keys from shares.
    pub fn get_peer_ids(&self) -> Vec<PublicKey> {
        self.shares
            .iter()
            .filter_map(|(key_hex, _)| PublicKey::from_hex(key_hex).ok())
            .collect()
    }

    /// Get the operations log link.
    pub fn ops_log(&self) -> Option<&Link> {
        self.ops_log.as_ref()
    }

    /// Set the operations log link.
    pub fn set_ops_log(&mut self, link: Link) {
        self.ops_log = Some(link);
    }

    /// Get mutable access to shares.
    pub fn shares_mut(&mut self) -> &mut BTreeMap<String, Share> {
        &mut self.shares
    }

    /// Remove a principal from the bucket.
    pub fn remove_principal(&mut self, public_key: &PublicKey) -> Option<Share> {
        self.shares.remove(&public_key.to_hex())
    }

    /// Get all mirror shares.
    pub fn get_mirrors(&self) -> Vec<&Share> {
        self.shares
            .values()
            .filter(|s| *s.role() == PrincipalRole::Mirror)
            .collect()
    }

    /// Get all owner shares.
    pub fn get_owners(&self) -> Vec<&Share> {
        self.shares
            .values()
            .filter(|s| *s.role() == PrincipalRole::Owner)
            .collect()
    }

    /// Check if the bucket is published.
    ///
    /// Published buckets have their secret stored in plaintext, allowing
    /// anyone with the manifest to decrypt contents.
    pub fn is_published(&self) -> bool {
        self.published_secret.is_some()
    }

    /// Get the published secret if available.
    pub fn published_secret(&self) -> Option<&Secret> {
        self.published_secret.as_ref()
    }

    /// Publish the bucket by storing the secret in plaintext.
    ///
    /// **Warning**: This is irreversible. Once published, the secret is exposed
    /// and anyone with the manifest can decrypt bucket contents.
    pub fn publish(&mut self, secret: &Secret) {
        self.published_secret = Some(secret.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::crypto::{PublicKey, Secret};

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
}
