use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::crypto::{PublicKey, Secret, SecretShare, SecretShareError};
use crate::linked_data::{BlockEncoded, DagCborCodec, Link};
use crate::version::Version;

use super::principal::{Principal, PrincipalRole};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Share {
    principal: Principal,
    share: SecretShare,
}

impl Share {
    pub fn new(share: SecretShare, public_key: PublicKey) -> Self {
        Self {
            principal: Principal {
                role: PrincipalRole::Owner,
                identity: public_key,
            },
            share,
        }
    }

    pub fn principal(&self) -> &Principal {
        &self.principal
    }

    pub fn share(&self) -> &SecretShare {
        &self.share
    }
}

pub type Shares = BTreeMap<String, Share>;

/**
* BucketData
* ==========
* BucketData is the serializable metadata for a bucket.
* It stores:
*   - an identifier for the bucket (global and static)
*   - a friendly name for the bucket (for display)
*   - shares (access control and encryption keys for principals)
*   - pins (optional pin set)
*   - previous version link
*   - version info
*/
#[allow(clippy::doc_overindented_list_items)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    // Buckets have a global unique identifier
    //  that clients should respect
    id: Uuid,
    // They also have a friendly name,
    // buckets are identified via unique pairs
    //  of <name, pk>
    name: String,
    // the set of principals that have access to the bucket
    //  and their roles
    // Using String as key for CBOR compatibility
    shares: Shares,
    // entry into the bucket
    entry: Link,
    // a pointer to a HashSeq block describing the pin set
    //  for the bucket
    pins: Link,
    // and a point to the previous version of the bucket
    previous: Option<Link>,
    // the height of this manifest in the bucket's version chain
    height: u64,
    // specify the software version as a sanity check
    version: Version,
}

impl BlockEncoded<DagCborCodec> for Manifest {}

impl Manifest {
    /// Create a new bucket with a name, owner, and share, and entry node link
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
                    share,
                },
            )]),
            entry,
            pins,
            previous: None,
            height,
            version: Version::default(),
        }
    }

    pub fn get_share(&self, public_key: &PublicKey) -> Option<&Share> {
        self.shares.get(&public_key.to_hex())
    }

    pub fn add_share(
        &mut self,
        public_key: PublicKey,
        secret: Secret,
    ) -> Result<(), SecretShareError> {
        let share = SecretShare::new(&secret, &public_key)?;
        let bucket_share = Share::new(share, public_key);
        self.shares.insert(public_key.to_hex(), bucket_share);
        Ok(())
    }

    pub fn unset_shares(&mut self) {
        self.shares.clear();
    }

    pub fn id(&self) -> &Uuid {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn shares(&self) -> &BTreeMap<String, Share> {
        &self.shares
    }

    pub fn version(&self) -> &Version {
        &self.version
    }

    pub fn entry(&self) -> &Link {
        &self.entry
    }

    pub fn set_entry(&mut self, entry: Link) {
        self.entry = entry;
    }

    pub fn pins(&self) -> &Link {
        &self.pins
    }

    pub fn set_pins(&mut self, pins_link: Link) {
        self.pins = pins_link;
    }

    pub fn set_previous(&mut self, previous: Link) {
        self.previous = Some(previous);
    }

    pub fn previous(&self) -> &Option<Link> {
        &self.previous
    }

    pub fn height(&self) -> u64 {
        self.height
    }

    pub fn set_height(&mut self, height: u64) {
        self.height = height;
    }

    /// Get all peer IDs from shares
    pub fn get_peer_ids(&self) -> Vec<PublicKey> {
        self.shares
            .iter()
            .filter_map(|(key_hex, _)| PublicKey::from_hex(key_hex).ok())
            .collect()
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
