use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::linked_data::Link;

/// Sync status between two peers for a bucket
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PingStatus {
    /// The peer does not have this bucket at all
    NotFound,
    /// We are behind the requesting peer's history,
    ///  which practically means we don't see the link
    ///  in our own history --- this does not mean the link
    ///  we are responding too is valid! it might be the requesting
    ///  peer is pinging for link which does not resolve to our
    ///  genesis!
    Behind(Link),
    /// Both agree on the current link (in sync)
    InSync,
    /// The ping illustrates a corruption of provenance, such as
    ///  what if I agree that the link is in my history, but we
    ///  disagree on height?
    OutOfSync,
    /// We are ahead of the current peer's history
    Ahead(Link),
}

/// Request to ping a peer and check bucket sync status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ping {
    /// The bucket ID to check
    pub bucket_id: Uuid,
    /// The current link the requesting peer has for this bucket
    pub link: Link,
    /// The height of the link we are responding to
    pub height: u64,
}

/// Response to a ping request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pong {
    pub status: PingStatus,
}
