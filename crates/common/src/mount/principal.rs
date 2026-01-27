//! # Principals
//!
//! Principals represent identities (public keys) with permissions on a bucket.
//!
//! Each principal has:
//! - An **identity** (Ed25519 public key)
//! - A **role** defining their access level ([`PrincipalRole`])
//!
//! ## Trust Model
//!
//! There is no cryptographic enforcement of roles. Role-based access control
//! is enforced by clients validating bucket updates against the prior state.
//! Only add principals you trust.
//!
//! ## Shares
//!
//! Principals may have an associated [`SecretShare`](crate::crypto::SecretShare)
//! allowing them to decrypt bucket content. The share is stored separately in
//! [`Share`](super::Share), not in the [`Principal`] struct itself.

use serde::{Deserialize, Serialize};

use crate::crypto::PublicKey;

/// The role of a principal on a bucket.
///
/// Roles determine what operations a principal can perform and when they
/// receive encryption access.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrincipalRole {
    /// Full read/write access to the bucket.
    ///
    /// Owners:
    /// - Always have an encrypted [`SecretShare`](crate::crypto::SecretShare)
    /// - Can modify bucket contents (add, remove, move files)
    /// - Can add/remove other principals
    /// - Can publish the bucket to grant mirror access
    Owner,

    /// Read-only access after publication.
    ///
    /// Mirrors:
    /// - Can sync bucket data (encrypted blobs) at any time
    /// - Cannot decrypt content until the bucket is published
    /// - Once published, read the plaintext secret from the manifest
    /// - Cannot modify bucket contents
    /// - Useful for CDN/gateway nodes that serve published content
    Mirror,
}

impl std::fmt::Display for PrincipalRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrincipalRole::Owner => write!(f, "Owner"),
            PrincipalRole::Mirror => write!(f, "Mirror"),
        }
    }
}

/// A principal identity on a bucket.
///
/// The principal struct contains the identity and role, but not the encryption
/// share. Shares are stored separately in [`Share`](super::Share) to allow
/// mirrors to exist without shares until publication.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Principal {
    /// The principal's access level.
    pub role: PrincipalRole,
    /// The principal's Ed25519 public key.
    pub identity: PublicKey,
}
