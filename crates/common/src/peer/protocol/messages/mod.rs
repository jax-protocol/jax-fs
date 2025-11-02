use serde::{Deserialize, Serialize};

mod announce;
mod ping;

pub use announce::Announce;
pub use ping::{Ping, PingStatus, Pong};

// TODO (amiller68): in an ideal world,
//  this module describes a generic 'handler'
//  that allows for
//  - handling incoming messages through either a single
//    or bi-directional stream
//  - defining peer behavior over state
// without much repetitive boilerplate.
//
// The way this is currently written is just *not* extensible
//  and i really hate it

/// Top-level request enum for the JAX protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Ping request to check sync status
    Ping(Ping),
    /// Fetch bucket request to get current link
    // FetchBucket(FetchBucketRequest),
    /// Announce message (one-way, no response expected)
    Announce(Announce),
}

/// Top-level response enum for the JAX protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Reply {
    /// Ping response with sync status
    Ping(Pong),
}
