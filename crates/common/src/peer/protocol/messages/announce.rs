use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::linked_data::Link;

/// Announcement of a new bucket version to peers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Announce {
    /// The bucket ID being announced
    pub bucket_id: Uuid,
    /// The new link for this bucket
    pub link: Link,
}
