//! Actor that handles iroh-blobs proto::Request commands.
//!
//! This actor bridges our SQLite + Object Storage backend to the iroh-blobs RPC protocol.

use std::{
    collections::{BTreeMap, HashSet},
    io,
    num::NonZeroU64,
    sync::atomic::{AtomicU64, Ordering},
};

use bao_tree::{
    io::{mixed::EncodedItem, outboard::PreOrderMemOutboard, BaoContentItem, Leaf},
    BaoTree, ChunkRanges,
};
use bytes::Bytes;
use iroh_blobs::{
    api::{
        proto::{
            AddProgressItem, BatchMsg, BatchResponse, BlobDeleteRequest, BlobStatus, BlobStatusMsg,
            BlobStatusRequest, Command, CreateTagMsg, CreateTagRequest, CreateTempTagMsg,
            DeleteBlobsMsg, DeleteTagsMsg, DeleteTagsRequest, ExportBaoMsg, ExportBaoRequest,
            ExportPathMsg, ExportPathRequest, ExportProgressItem, ExportRangesItem,
            ExportRangesMsg, ExportRangesRequest, ImportBaoMsg, ImportBaoRequest,
            ImportByteStreamMsg, ImportByteStreamUpdate, ImportBytesMsg, ImportBytesRequest,
            ImportPathMsg, ImportPathRequest, ListBlobsMsg, ListTagsMsg, ListTagsRequest,
            ObserveMsg, ObserveRequest, RenameTagMsg, RenameTagRequest, Scope, SetTagMsg,
            SetTagRequest, ShutdownMsg, TagInfo,
        },
        Error as ApiError, Tag, TempTag,
    },
    BlobFormat, Hash, HashAndFormat,
};
use irpc::channel::mpsc;
use range_collections::range_set::RangeSetRange;
use tokio::{
    io::AsyncReadExt,
    task::{JoinError, JoinSet},
};
use tracing::{debug, error, info, trace, warn, Instrument};

use crate::object_store::BlobStore;

/// Block size for BAO tree operations (matches iroh-blobs IROH_BLOCK_SIZE)
const IROH_BLOCK_SIZE: bao_tree::BlockSize = bao_tree::BlockSize::from_chunk_log(4);

/// Result type for task operations
enum TaskResult {
    Unit(()),
    Import(anyhow::Result<ImportEntry>),
    Scope(Scope),
}

impl From<()> for TaskResult {
    fn from(_: ()) -> Self {
        TaskResult::Unit(())
    }
}

impl From<anyhow::Result<ImportEntry>> for TaskResult {
    fn from(res: anyhow::Result<ImportEntry>) -> Self {
        TaskResult::Import(res)
    }
}

impl From<Scope> for TaskResult {
    fn from(scope: Scope) -> Self {
        TaskResult::Scope(scope)
    }
}

/// Data for a pending import operation
struct ImportEntry {
    scope: Scope,
    format: BlobFormat,
    data: Bytes,
    outboard: PreOrderMemOutboard,
    tx: mpsc::Sender<AddProgressItem>,
}

/// Simple in-memory temp tag tracking
#[derive(Default)]
struct TempTagManager {
    next_id: AtomicU64,
    tags: BTreeMap<u64, HashAndFormat>,
}

impl TempTagManager {
    fn create(&mut self, _scope: Scope, value: HashAndFormat) -> TempTag {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.tags.insert(id, value);
        // TempTag::new takes (value, on_drop) - we don't have a real TagDrop impl
        TempTag::new(value, None)
    }

    fn list(&self) -> Vec<HashAndFormat> {
        self.tags.values().cloned().collect()
    }

    fn create_scope(&mut self) -> (Scope, u64) {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        (Scope::default(), id)
    }

    fn end_scope(&mut self, _scope: Scope) {
        // In a full impl, we'd track which tags belong to which scope
    }
}

/// Actor that handles iroh-blobs protocol commands using our SQLite + S3 backend.
pub struct ObjectStoreActor {
    /// Receiver for incoming commands
    commands: tokio::sync::mpsc::Receiver<Command>,
    /// Background tasks
    tasks: JoinSet<TaskResult>,
    /// Our blob store backend
    store: BlobStore,
    /// In-memory tag storage (name -> hash+format)
    tags: BTreeMap<Tag, HashAndFormat>,
    /// Temp tags manager
    temp_tags: TempTagManager,
    /// Protected hashes (not deleted during GC)
    protected: HashSet<Hash>,
    /// Waiters for idle state
    idle_waiters: Vec<irpc::channel::oneshot::Sender<()>>,
}

impl ObjectStoreActor {
    /// Create a new actor with the given blob store.
    pub fn new(store: BlobStore, commands: tokio::sync::mpsc::Receiver<Command>) -> Self {
        Self {
            commands,
            tasks: JoinSet::new(),
            store,
            tags: BTreeMap::new(),
            temp_tags: TempTagManager::default(),
            protected: HashSet::new(),
            idle_waiters: Vec::new(),
        }
    }

    /// Run the actor loop.
    pub async fn run(mut self) {
        let shutdown = loop {
            tokio::select! {
                cmd = self.commands.recv() => {
                    let Some(cmd) = cmd else {
                        // Last sender dropped, exit
                        break None;
                    };
                    if let Some(cmd) = self.handle_command(cmd).await {
                        break Some(cmd);
                    }
                }
                Some(res) = self.tasks.join_next(), if !self.tasks.is_empty() => {
                    let Some(res) = self.log_task_result(res) else {
                        continue;
                    };
                    match res {
                        TaskResult::Import(res) => {
                            self.finish_import(res).await;
                        }
                        TaskResult::Scope(scope) => {
                            self.temp_tags.end_scope(scope);
                        }
                        TaskResult::Unit(_) => {}
                    }
                    if self.tasks.is_empty() {
                        // We are idle now
                        for tx in self.idle_waiters.drain(..) {
                            tx.send(()).await.ok();
                        }
                    }
                }
            }
        };
        if let Some(shutdown) = shutdown {
            shutdown.tx.send(()).await.ok();
        }
    }

    fn spawn<F, T>(&mut self, f: F)
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Into<TaskResult>,
    {
        let span = tracing::Span::current();
        let fut = async move { f.await.into() }.instrument(span);
        self.tasks.spawn(fut);
    }

    fn log_task_result(&self, res: Result<TaskResult, JoinError>) -> Option<TaskResult> {
        match res {
            Ok(x) => Some(x),
            Err(e) => {
                if e.is_cancelled() {
                    trace!("task cancelled: {e}");
                } else {
                    error!("task failed: {e}");
                }
                None
            }
        }
    }

    async fn handle_command(&mut self, cmd: Command) -> Option<ShutdownMsg> {
        match cmd {
            // Blob status check
            Command::BlobStatus(cmd) => {
                let BlobStatusMsg {
                    inner: BlobStatusRequest { hash },
                    tx,
                    ..
                } = cmd;
                let store = self.store.clone();
                self.spawn(async move {
                    let status = match store.get(&hash).await {
                        Ok(Some(data)) => BlobStatus::Complete {
                            size: data.len() as u64,
                        },
                        Ok(None) => BlobStatus::NotFound,
                        Err(e) => {
                            warn!("BlobStatus error: {e}");
                            BlobStatus::NotFound
                        }
                    };
                    tx.send(status).await.ok();
                });
            }

            // List all blobs
            Command::ListBlobs(cmd) => {
                let ListBlobsMsg { tx, .. } = cmd;
                let store = self.store.clone();
                self.spawn(async move {
                    match store.list().await {
                        Ok(hashes) => {
                            for hash in hashes {
                                if tx.send(Ok(hash)).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            warn!("ListBlobs error: {e}");
                        }
                    }
                });
            }

            // Delete blobs
            Command::DeleteBlobs(cmd) => {
                let DeleteBlobsMsg {
                    inner: BlobDeleteRequest { hashes, force },
                    tx,
                    ..
                } = cmd;
                let store = self.store.clone();
                let protected = self.protected.clone();
                self.spawn(async move {
                    for hash in hashes {
                        if !force && protected.contains(&hash) {
                            continue;
                        }
                        if let Err(e) = store.delete(&hash).await {
                            warn!("DeleteBlobs error for {}: {e}", hash);
                        }
                    }
                    tx.send(Ok(())).await.ok();
                });
            }

            // Import bytes (complete data at once)
            Command::ImportBytes(cmd) => {
                let ImportBytesMsg {
                    inner:
                        ImportBytesRequest {
                            data,
                            scope,
                            format,
                            ..
                        },
                    tx,
                    ..
                } = cmd;
                self.spawn(import_bytes(data, scope, format, tx));
            }

            // Import byte stream (chunked)
            Command::ImportByteStream(cmd) => {
                let ImportByteStreamMsg { inner, tx, rx, .. } = cmd;
                self.spawn(import_byte_stream(inner.scope, inner.format, rx, tx));
            }

            // Import from path
            Command::ImportPath(cmd) => {
                self.spawn(import_path(cmd));
            }

            // Import BAO (verified streaming)
            Command::ImportBao(cmd) => {
                let ImportBaoMsg {
                    inner: ImportBaoRequest { hash, size },
                    rx,
                    tx,
                    ..
                } = cmd;
                let store = self.store.clone();
                self.spawn(import_bao(store, hash, size, rx, tx));
            }

            // Export BAO
            Command::ExportBao(cmd) => {
                let ExportBaoMsg {
                    inner: ExportBaoRequest { hash, ranges },
                    tx,
                    ..
                } = cmd;
                let store = self.store.clone();
                self.spawn(export_bao(store, hash, ranges, tx));
            }

            // Export ranges (for HTTP serving)
            Command::ExportRanges(cmd) => {
                let store = self.store.clone();
                self.spawn(export_ranges(store, cmd));
            }

            // Export to path
            Command::ExportPath(cmd) => {
                let store = self.store.clone();
                self.spawn(export_path(store, cmd));
            }

            // Observe bitfield changes
            Command::Observe(cmd) => {
                let ObserveMsg {
                    inner: ObserveRequest { hash },
                    tx,
                    ..
                } = cmd;
                let store = self.store.clone();
                self.spawn(observe(store, hash, tx));
            }

            // Tag operations
            Command::ListTags(cmd) => {
                let ListTagsMsg {
                    inner:
                        ListTagsRequest {
                            from,
                            to,
                            raw,
                            hash_seq,
                        },
                    tx,
                    ..
                } = cmd;
                let tags: Vec<_> = self
                    .tags
                    .iter()
                    .filter(move |(tag, value)| {
                        if let Some(ref from) = from {
                            if *tag < from {
                                return false;
                            }
                        }
                        if let Some(ref to) = to {
                            if *tag >= to {
                                return false;
                            }
                        }
                        raw && value.format.is_raw() || hash_seq && value.format.is_hash_seq()
                    })
                    .map(|(tag, value)| TagInfo {
                        name: tag.clone(),
                        hash: value.hash,
                        format: value.format,
                    })
                    .map(Ok)
                    .collect();
                tx.send(tags).await.ok();
            }

            Command::SetTag(cmd) => {
                let SetTagMsg {
                    inner: SetTagRequest { name: tag, value },
                    tx,
                    ..
                } = cmd;
                self.tags.insert(tag, value);
                tx.send(Ok(())).await.ok();
            }

            Command::DeleteTags(cmd) => {
                let DeleteTagsMsg {
                    inner: DeleteTagsRequest { from, to },
                    tx,
                    ..
                } = cmd;
                let mut deleted = 0u64;
                self.tags.retain(|tag, _| {
                    if let Some(ref from) = from {
                        if tag < from {
                            return true;
                        }
                    }
                    if let Some(ref to) = to {
                        if tag >= to {
                            return true;
                        }
                    }
                    deleted += 1;
                    false
                });
                tx.send(Ok(deleted)).await.ok();
            }

            Command::RenameTag(cmd) => {
                let RenameTagMsg {
                    inner: RenameTagRequest { from, to },
                    tx,
                    ..
                } = cmd;
                match self.tags.remove(&from) {
                    Some(value) => {
                        self.tags.insert(to, value);
                        tx.send(Ok(())).await.ok();
                    }
                    None => {
                        tx.send(Err(ApiError::io(
                            io::ErrorKind::NotFound,
                            format!("tag not found: {from:?}"),
                        )))
                        .await
                        .ok();
                    }
                }
            }

            Command::CreateTag(cmd) => {
                let CreateTagMsg {
                    inner: CreateTagRequest { value },
                    tx,
                    ..
                } = cmd;
                let tag = Tag::auto(std::time::SystemTime::now(), |tag| {
                    self.tags.contains_key(tag)
                });
                self.tags.insert(tag.clone(), value);
                tx.send(Ok(tag)).await.ok();
            }

            // Temp tag operations
            Command::CreateTempTag(cmd) => {
                let CreateTempTagMsg { tx, inner, .. } = cmd;
                let tt = self.temp_tags.create(inner.scope, inner.value);
                tx.send(tt).await.ok();
            }

            Command::ListTempTags(cmd) => {
                let tts = self.temp_tags.list();
                cmd.tx.send(tts).await.ok();
            }

            // Batch operation
            Command::Batch(cmd) => {
                let (id, _scope_id) = self.temp_tags.create_scope();
                self.spawn(handle_batch(cmd, id));
            }

            // Lifecycle operations
            Command::SyncDb(cmd) => {
                // For S3-backed store, sync is a no-op (S3 is immediately consistent)
                cmd.tx.send(Ok(())).await.ok();
            }

            Command::WaitIdle(cmd) => {
                if self.tasks.is_empty() {
                    cmd.tx.send(()).await.ok();
                } else {
                    self.idle_waiters.push(cmd.tx);
                }
            }

            Command::Shutdown(cmd) => {
                return Some(cmd);
            }

            Command::ClearProtected(cmd) => {
                self.protected.clear();
                cmd.tx.send(Ok(())).await.ok();
            }
        }
        None
    }

    async fn finish_import(&mut self, res: anyhow::Result<ImportEntry>) {
        let import_data = match res {
            Ok(entry) => entry,
            Err(e) => {
                error!("import failed: {e}");
                return;
            }
        };

        let hash: Hash = import_data.outboard.root.into();

        // Store the data
        match self.store.put(import_data.data.to_vec()).await {
            Ok(stored_hash) => {
                debug_assert_eq!(hash, stored_hash, "hash mismatch during import");

                // Create temp tag
                let tt = self.temp_tags.create(
                    import_data.scope,
                    HashAndFormat {
                        hash,
                        format: import_data.format,
                    },
                );
                import_data.tx.send(AddProgressItem::Done(tt)).await.ok();
            }
            Err(e) => {
                error!("failed to store imported data: {e}");
                import_data
                    .tx
                    .send(AddProgressItem::Error(io::Error::other(e.to_string())))
                    .await
                    .ok();
            }
        }
    }
}

// Task implementations

async fn import_bytes(
    data: Bytes,
    scope: Scope,
    format: BlobFormat,
    tx: mpsc::Sender<AddProgressItem>,
) -> anyhow::Result<ImportEntry> {
    tx.send(AddProgressItem::Size(data.len() as u64)).await?;
    tx.send(AddProgressItem::CopyDone).await?;
    let outboard = PreOrderMemOutboard::create(&data, IROH_BLOCK_SIZE);
    Ok(ImportEntry {
        data,
        outboard,
        scope,
        format,
        tx,
    })
}

async fn import_byte_stream(
    scope: Scope,
    format: BlobFormat,
    mut rx: mpsc::Receiver<ImportByteStreamUpdate>,
    tx: mpsc::Sender<AddProgressItem>,
) -> anyhow::Result<ImportEntry> {
    let mut res = Vec::new();
    loop {
        match rx.recv().await {
            Ok(Some(ImportByteStreamUpdate::Bytes(data))) => {
                res.extend_from_slice(&data);
                tx.send(AddProgressItem::CopyProgress(res.len() as u64))
                    .await?;
            }
            Ok(Some(ImportByteStreamUpdate::Done)) => {
                break;
            }
            Ok(None) => {
                return Err(ApiError::io(
                    io::ErrorKind::UnexpectedEof,
                    "byte stream ended unexpectedly",
                )
                .into());
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }
    import_bytes(res.into(), scope, format, tx).await
}

async fn import_path(cmd: ImportPathMsg) -> anyhow::Result<ImportEntry> {
    let ImportPathMsg {
        inner:
            ImportPathRequest {
                path,
                scope,
                format,
                ..
            },
        tx,
        ..
    } = cmd;
    let mut res = Vec::new();
    let mut file = tokio::fs::File::open(&path).await?;
    let mut buf = [0u8; 1024 * 64];
    loop {
        let size = file.read(&mut buf).await?;
        if size == 0 {
            break;
        }
        res.extend_from_slice(&buf[..size]);
        tx.send(AddProgressItem::CopyProgress(res.len() as u64))
            .await?;
    }
    import_bytes(res.into(), scope, format, tx).await
}

/// Maximum blob size we're willing to import (1 GB).
/// This prevents memory exhaustion attacks from malformed or malicious size values.
const MAX_IMPORT_SIZE: u64 = 1024 * 1024 * 1024;

async fn import_bao(
    store: BlobStore,
    hash: Hash,
    size: NonZeroU64,
    mut rx: mpsc::Receiver<BaoContentItem>,
    tx: irpc::channel::oneshot::Sender<iroh_blobs::api::Result<()>>,
) {
    let size = size.get();
    debug!("ImportBao: starting import for hash {} size {}", hash, size);

    // Sanity check: reject absurdly large sizes to prevent OOM
    if size > MAX_IMPORT_SIZE {
        error!(
            "ImportBao: rejecting import of hash {} with unreasonable size {} (max is {})",
            hash, size, MAX_IMPORT_SIZE
        );
        tx.send(Err(ApiError::io(
            io::ErrorKind::InvalidInput,
            format!("blob size {} exceeds maximum {}", size, MAX_IMPORT_SIZE),
        )))
        .await
        .ok();
        return;
    }

    let mut data = vec![0u8; size as usize];
    let mut leaf_count = 0usize;
    let mut parent_count = 0usize;
    let mut bytes_received = 0usize;

    let tree = BaoTree::new(size, IROH_BLOCK_SIZE);
    // Calculate outboard size: (chunks - 1) * 64 bytes where each chunk is 16KB
    // For the block size 2^4, chunk size is 16KB
    let chunk_size = 1024u64 * 16; // 16KB
    let chunks = size.div_ceil(chunk_size);
    let outboard_size = if chunks > 1 { (chunks - 1) * 64 } else { 0 };
    let mut outboard = vec![0u8; outboard_size as usize];

    while let Ok(Some(item)) = rx.recv().await {
        match item {
            BaoContentItem::Parent(parent) => {
                parent_count += 1;
                if let Some(offset) = tree.pre_order_offset(parent.node) {
                    let mut pair = [0u8; 64];
                    pair[..32].copy_from_slice(parent.pair.0.as_bytes());
                    pair[32..].copy_from_slice(parent.pair.1.as_bytes());
                    let start = (offset * 64) as usize;
                    if start + 64 <= outboard.len() {
                        outboard[start..start + 64].copy_from_slice(&pair);
                    }
                }
            }
            BaoContentItem::Leaf(leaf) => {
                leaf_count += 1;
                let start = leaf.offset as usize;
                let end = start + leaf.data.len();
                bytes_received += leaf.data.len();
                if end <= data.len() {
                    data[start..end].copy_from_slice(&leaf.data);
                }
            }
        }
    }

    debug!(
        "ImportBao: received {} leaves, {} parents, {} bytes for hash {}",
        leaf_count, parent_count, bytes_received, hash
    );

    // Verify and store
    let computed_hash = Hash::new(&data);
    if computed_hash != hash {
        warn!(
            "ImportBao hash mismatch: expected {}, got {} (received {} bytes, {} leaves)",
            hash, computed_hash, bytes_received, leaf_count
        );
        tx.send(Err(ApiError::io(
            io::ErrorKind::InvalidData,
            "hash mismatch",
        )))
        .await
        .ok();
        return;
    }

    debug!(
        "ImportBao: hash verified, storing {} bytes for {}",
        size, hash
    );

    match store.put(data).await {
        Ok(stored_hash) => {
            info!(
                "ImportBao: successfully stored hash {} (returned {})",
                hash, stored_hash
            );
            tx.send(Ok(())).await.ok();
        }
        Err(e) => {
            error!("ImportBao: failed to store hash {}: {}", hash, e);
            tx.send(Err(ApiError::other(e.to_string()))).await.ok();
        }
    }
}

async fn export_bao(
    store: BlobStore,
    hash: Hash,
    _ranges: ChunkRanges,
    tx: mpsc::Sender<EncodedItem>,
) {
    let data = match store.get(&hash).await {
        Ok(Some(data)) => data,
        Ok(None) => {
            tx.send(EncodedItem::Error(bao_tree::io::EncodeError::Io(
                io::Error::new(io::ErrorKind::NotFound, "hash not found"),
            )))
            .await
            .ok();
            return;
        }
        Err(e) => {
            tx.send(EncodedItem::Error(bao_tree::io::EncodeError::Io(
                io::Error::other(e.to_string()),
            )))
            .await
            .ok();
            return;
        }
    };

    // Create outboard for the data
    let outboard = PreOrderMemOutboard::create(&data, IROH_BLOCK_SIZE);

    // For simplicity, send all data as leaves
    // A full implementation would use traverse_ranges_validated
    let chunk_size = 1024u64;
    let mut offset = 0u64;
    while offset < data.len() as u64 {
        let end = (offset + chunk_size).min(data.len() as u64);
        let leaf = Leaf {
            offset,
            data: data.slice(offset as usize..end as usize),
        };
        if tx.send(EncodedItem::Leaf(leaf)).await.is_err() {
            break;
        }
        offset = end;
    }

    // Suppress unused variable warning
    let _ = outboard;
}

async fn export_ranges(store: BlobStore, cmd: ExportRangesMsg) {
    let ExportRangesRequest { hash, ranges } = cmd.inner;

    let data = match store.get(&hash).await {
        Ok(Some(data)) => data,
        Ok(None) => {
            cmd.tx
                .send(ExportRangesItem::Error(ApiError::io(
                    io::ErrorKind::NotFound,
                    "hash not found",
                )))
                .await
                .ok();
            return;
        }
        Err(e) => {
            cmd.tx
                .send(ExportRangesItem::Error(ApiError::other(e.to_string())))
                .await
                .ok();
            return;
        }
    };

    let size = data.len() as u64;

    // Send size first
    if cmd.tx.send(ExportRangesItem::Size(size)).await.is_err() {
        return;
    }

    // Send requested ranges
    for range in ranges.iter() {
        let (start, end) = match range {
            RangeSetRange::Range(r) => ((*r.start).min(size), (*r.end).min(size)),
            RangeSetRange::RangeFrom(r) => ((*r.start).min(size), size),
        };

        if start >= end {
            continue;
        }

        // Send in chunks
        const CHUNK_SIZE: u64 = 1024;
        let mut offset = start;
        while offset < end {
            let chunk_end = (offset + CHUNK_SIZE).min(end);
            let chunk_data = data.slice(offset as usize..chunk_end as usize);
            let leaf = Leaf {
                offset,
                data: chunk_data,
            };
            if cmd.tx.send(ExportRangesItem::Data(leaf)).await.is_err() {
                return;
            }
            offset = chunk_end;
        }
    }
}

async fn export_path(store: BlobStore, cmd: ExportPathMsg) {
    let ExportPathMsg { inner, tx, .. } = cmd;
    let ExportPathRequest { hash, target, .. } = inner;

    let data = match store.get(&hash).await {
        Ok(Some(data)) => data,
        Ok(None) => {
            tx.send(ExportProgressItem::Error(ApiError::io(
                io::ErrorKind::NotFound,
                "hash not found",
            )))
            .await
            .ok();
            return;
        }
        Err(e) => {
            tx.send(ExportProgressItem::Error(ApiError::other(e.to_string())))
                .await
                .ok();
            return;
        }
    };

    let size = data.len() as u64;
    tx.send(ExportProgressItem::Size(size)).await.ok();

    // Create parent directories
    if let Some(parent) = target.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            tx.send(ExportProgressItem::Error(ApiError::io(
                io::ErrorKind::Other,
                e.to_string(),
            )))
            .await
            .ok();
            return;
        }
    }

    // Write the file
    match tokio::fs::write(&target, &data).await {
        Ok(()) => {
            tx.send(ExportProgressItem::Done).await.ok();
        }
        Err(e) => {
            tx.send(ExportProgressItem::Error(ApiError::io(
                io::ErrorKind::Other,
                e.to_string(),
            )))
            .await
            .ok();
        }
    }
}

async fn observe(store: BlobStore, hash: Hash, tx: mpsc::Sender<iroh_blobs::api::blobs::Bitfield>) {
    // Check current status
    let bitfield = match store.get(&hash).await {
        Ok(Some(data)) => iroh_blobs::api::blobs::Bitfield::complete(data.len() as u64),
        Ok(None) => iroh_blobs::api::blobs::Bitfield::empty(),
        Err(_) => iroh_blobs::api::blobs::Bitfield::empty(),
    };

    // Send initial state
    tx.send(bitfield).await.ok();

    // For now, we don't support live updates - the observer will get the initial state
    // A full implementation would need to track in-progress imports and notify observers
}

async fn handle_batch(cmd: BatchMsg, id: Scope) -> Scope {
    if let Err(cause) = handle_batch_impl(cmd, id).await {
        error!("batch failed: {cause}");
    }
    id
}

async fn handle_batch_impl(cmd: BatchMsg, id: Scope) -> iroh_blobs::api::Result<()> {
    let BatchMsg { tx, mut rx, .. } = cmd;
    tx.send(id).await.map_err(ApiError::other)?;
    while let Some(msg) = rx.recv().await? {
        match msg {
            BatchResponse::Drop(_msg) => {
                // In a full impl, we'd remove the temp tag protection
            }
            BatchResponse::Ping => {}
        }
    }
    Ok(())
}
