//! FUSE filesystem implementation for jax-bucket
//!
//! Implements the fuser::Filesystem trait to expose bucket contents as a local filesystem.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEmpty,
    ReplyEntry, ReplyOpen, ReplyWrite, ReplyXattr, Request, TimeOrNow,
};
use libc;
use tokio::runtime::Handle;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::fuse::cache::{CachedAttr, CachedContent, CachedDirEntry, FileCache, FileCacheConfig};
use crate::fuse::inode_table::InodeTable;
use crate::fuse::sync_events::{SaveRequest, SyncEvent};
use common::mount::Mount;
use tokio::sync::mpsc;

/// Write buffer for pending writes
#[derive(Debug)]
struct WriteBuffer {
    data: Vec<u8>,
    dirty: bool,
}

/// FUSE filesystem for a jax bucket
pub struct JaxFs {
    /// Tokio runtime handle for async operations
    rt: Handle,
    /// Direct mount reference (no HTTP)
    mount: Arc<RwLock<Mount>>,
    /// Mount ID
    mount_id: Uuid,
    /// Bucket ID
    bucket_id: Uuid,
    /// Inode table
    inodes: RwLock<InodeTable>,
    /// Write buffers: file handle → buffer
    write_buffers: RwLock<HashMap<u64, WriteBuffer>>,
    /// File cache
    cache: FileCache,
    /// Sync event receiver (used when sync listener is spawned)
    #[allow(dead_code)]
    sync_rx: Option<broadcast::Receiver<SyncEvent>>,
    /// Save request sender - sends requests to MountManager when flush succeeds
    save_tx: Option<mpsc::Sender<SaveRequest>>,
    /// Read-only mode
    read_only: bool,
    /// Next file handle
    next_fh: std::sync::atomic::AtomicU64,
}

impl JaxFs {
    /// Default TTL for FUSE attributes
    const ATTR_TTL: Duration = Duration::from_secs(1);

    /// Block size for FUSE
    const BLOCK_SIZE: u32 = 512;

    /// Create a new JaxFs
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rt: Handle,
        mount: Arc<RwLock<Mount>>,
        mount_id: Uuid,
        bucket_id: Uuid,
        cache_config: FileCacheConfig,
        read_only: bool,
        sync_rx: Option<broadcast::Receiver<SyncEvent>>,
        save_tx: Option<mpsc::Sender<SaveRequest>>,
    ) -> Self {
        Self {
            rt,
            mount,
            mount_id,
            bucket_id,
            inodes: RwLock::new(InodeTable::new()),
            write_buffers: RwLock::new(HashMap::new()),
            cache: FileCache::new(cache_config),
            sync_rx,
            save_tx,
            read_only,
            next_fh: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Start the background sync listener
    pub fn spawn_sync_listener(&self, mut rx: broadcast::Receiver<SyncEvent>) {
        let cache = self.cache.clone();
        let mount_id = self.mount_id;

        std::thread::spawn(move || {
            // Create a mini runtime for the sync listener
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create sync listener runtime");

            rt.block_on(async move {
                loop {
                    match rx.recv().await {
                        Ok(SyncEvent::MountInvalidated { mount_id: id }) => {
                            if id == mount_id {
                                tracing::debug!("FUSE cache invalidated for mount {}", mount_id);
                                cache.invalidate_all();
                            }
                        }
                        Ok(SyncEvent::BucketUpdated { bucket_id }) => {
                            tracing::debug!("Bucket {} updated, invalidating cache", bucket_id);
                            cache.invalidate_all();
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("Sync listener lagged {} events", n);
                            cache.invalidate_all();
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            tracing::info!("Sync channel closed, stopping listener");
                            break;
                        }
                    }
                }
            });
        });
    }

    /// Generate the next file handle
    fn next_handle(&self) -> u64 {
        self.next_fh
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// Check if a filename should be filtered (macOS resource forks, etc.)
    fn should_filter(name: &str) -> bool {
        name.starts_with("._") || name == ".DS_Store" || name == ".Spotlight-V100"
    }

    /// Create a FileAttr from cached attributes
    fn make_attr(inode: u64, attr: &CachedAttr) -> FileAttr {
        let kind = if attr.is_dir {
            FileType::Directory
        } else {
            FileType::RegularFile
        };

        let mtime = UNIX_EPOCH + Duration::from_secs(attr.mtime as u64);
        let perm = if attr.is_dir { 0o755 } else { 0o644 };

        FileAttr {
            ino: inode,
            size: attr.size,
            blocks: attr.size.div_ceil(Self::BLOCK_SIZE as u64),
            atime: mtime,
            mtime,
            ctime: mtime,
            crtime: mtime,
            kind,
            perm,
            nlink: 1,
            uid: unsafe { libc::getuid() },
            gid: unsafe { libc::getgid() },
            rdev: 0,
            blksize: Self::BLOCK_SIZE,
            flags: 0,
        }
    }

    /// Fetch attributes for a path via Mount
    fn fetch_attr(&self, path: &str) -> Option<CachedAttr> {
        // Check cache first
        if let Some(attr) = self.cache.get_attr(path) {
            return Some(attr);
        }

        let mount = self.mount.clone();
        let path_str = path.to_string();
        let cache_path = path_str.clone();

        let result = self.rt.block_on(async move {
            // For root, we know it's a directory
            if path_str == "/" {
                return Some(CachedAttr {
                    size: 0,
                    is_dir: true,
                    mime_type: None,
                    mtime: chrono::Utc::now().timestamp(),
                });
            }

            // For non-root, check if it's in parent directory listing
            let parent = InodeTable::parent_path(&path_str);
            let filename = InodeTable::filename(&path_str);

            let parent_path = std::path::Path::new(&parent);
            let mount_guard = mount.read().await;
            let entries = mount_guard.ls(parent_path).await.ok()?;

            for (entry_path, link) in entries {
                if let Some(name) = entry_path.file_name() {
                    if name.to_string_lossy() == filename {
                        // Check if it's a directory using the link type
                        let is_dir = link.is_dir();
                        // For files, we need to fetch content to get actual size
                        // For now, use a large fake size and let read() handle EOF
                        let size = if is_dir {
                            0
                        } else {
                            // Fetch content to get actual size
                            let abs_path = std::path::PathBuf::from("/").join(&entry_path);
                            match mount_guard.cat(&abs_path).await {
                                Ok(data) => data.len() as u64,
                                Err(_) => 0,
                            }
                        };
                        return Some(CachedAttr {
                            size,
                            is_dir,
                            mime_type: None,
                            mtime: chrono::Utc::now().timestamp(),
                        });
                    }
                }
            }

            None
        });

        if let Some(ref attr) = result {
            self.cache.put_attr(&cache_path, attr.clone());
        }

        result
    }

    /// Fetch directory listing via Mount
    fn fetch_dir(&self, path: &str) -> Option<Vec<CachedDirEntry>> {
        // Check cache first
        if let Some(entries) = self.cache.get_dir(path) {
            tracing::debug!("FUSE fetch_dir cache hit for path: {}", path);
            return Some(entries);
        }

        let mount = self.mount.clone();
        let fs_path = std::path::Path::new(path);
        let cache_path = path.to_string();

        tracing::debug!("FUSE fetch_dir via Mount: path={}", path);

        let result = self.rt.block_on(async move {
            let mount_guard = mount.read().await;
            match mount_guard.ls(fs_path).await {
                Ok(entries_map) => {
                    tracing::debug!("FUSE fetch_dir got {} entries", entries_map.len());
                    let entries: Vec<CachedDirEntry> = entries_map
                        .into_iter()
                        .filter_map(|(entry_path, link)| {
                            let name = entry_path.file_name()?.to_string_lossy().to_string();
                            if Self::should_filter(&name) {
                                return None;
                            }
                            // Check if it's a directory using the link type
                            let is_dir = link.is_dir();
                            Some(CachedDirEntry { name, is_dir })
                        })
                        .collect();
                    Some(entries)
                }
                Err(e) => {
                    tracing::error!("FUSE fetch_dir Mount error: {:?}", e);
                    None
                }
            }
        });

        if let Some(ref entries) = result {
            self.cache.put_dir(&cache_path, entries.clone());
        }

        result
    }

    /// Fetch file content via Mount
    fn fetch_content(&self, path: &str) -> Option<CachedContent> {
        // Check cache first
        if let Some(content) = self.cache.get_content(path) {
            return Some(content);
        }

        let mount = self.mount.clone();
        let fs_path = std::path::Path::new(path);
        let cache_path = path.to_string();

        let result = self.rt.block_on(async move {
            let mount_guard = mount.read().await;
            match mount_guard.cat(fs_path).await {
                Ok(data) => Some(CachedContent {
                    data: Arc::new(data),
                    mime_type: "application/octet-stream".to_string(),
                }),
                Err(e) => {
                    tracing::error!("FUSE fetch_content Mount error: {:?}", e);
                    None
                }
            }
        });

        if let Some(ref content) = result {
            self.cache.put_content(&cache_path, content.clone());
        }

        result
    }

    /// Handle truncate operation (size parameter in setattr)
    fn handle_truncate(&self, path: &str, size: u64, fh: Option<u64>) -> Result<(), libc::c_int> {
        // Check if this is a directory
        if let Some(attr) = self.fetch_attr(path) {
            if attr.is_dir {
                return Err(libc::EISDIR);
            }
        }

        // Check write buffers first if we have a file handle
        if let Some(fh) = fh {
            let mut buffers = self.rt.block_on(self.write_buffers.write());
            if let Some(buffer) = buffers.get_mut(&fh) {
                buffer.data.resize(size as usize, 0);
                buffer.dirty = true;
                return Ok(());
            }
        }

        // No active write buffer - need to read-modify-write via Mount
        let mount = self.mount.clone();
        let path_str = path.to_string();

        let result: Result<(), libc::c_int> = self.rt.block_on(async move {
            let path_buf = std::path::PathBuf::from(&path_str);

            if size == 0 {
                // Truncate to empty: just write empty content
                let mut mount_guard = mount.write().await;
                mount_guard
                    .add(&path_buf, std::io::Cursor::new(Vec::new()))
                    .await
                    .map_err(|e| {
                        tracing::error!("Failed to truncate {}: {}", path_str, e);
                        libc::EIO
                    })?;
            } else {
                // Truncate to non-zero size: read current content, resize, write back
                let current_data = {
                    let mount_guard = mount.read().await;
                    mount_guard.cat(&path_buf).await.unwrap_or_default()
                };

                let mut new_data = current_data;
                new_data.resize(size as usize, 0);

                let mut mount_guard = mount.write().await;
                mount_guard
                    .add(&path_buf, std::io::Cursor::new(new_data))
                    .await
                    .map_err(|e| {
                        tracing::error!("Failed to truncate {}: {}", path_str, e);
                        libc::EIO
                    })?;
            }

            Ok(())
        });
        result?;

        // Invalidate cache after successful truncate
        self.cache.invalidate(path);

        // Request save to persist changes
        if let Some(ref save_tx) = self.save_tx {
            let mount_id = self.mount_id;
            let tx = save_tx.clone();
            self.rt.spawn(async move {
                if let Err(e) = tx.send(SaveRequest { mount_id }).await {
                    tracing::error!("Failed to send save request: {}", e);
                }
            });
        }

        Ok(())
    }
}

impl Filesystem for JaxFs {
    fn init(
        &mut self,
        _req: &Request<'_>,
        _config: &mut fuser::KernelConfig,
    ) -> Result<(), libc::c_int> {
        tracing::info!(
            "FUSE filesystem initialized for mount {} (bucket {})",
            self.mount_id,
            self.bucket_id
        );
        Ok(())
    }

    fn destroy(&mut self) {
        tracing::info!("FUSE filesystem destroyed for mount {}", self.mount_id);
    }

    fn lookup(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let name = match name.to_str() {
            Some(n) => n,
            None => {
                reply.error(libc::ENOENT);
                return;
            }
        };

        // Filter macOS special files
        if Self::should_filter(name) {
            reply.error(libc::ENOENT);
            return;
        }

        // Get parent path
        let parent_path = {
            let inodes = self.rt.block_on(self.inodes.read());
            match inodes.get_path(parent) {
                Some(p) => p.to_string(),
                None => {
                    reply.error(libc::ENOENT);
                    return;
                }
            }
        };

        // Build full path
        let path = if parent_path == "/" {
            format!("/{}", name)
        } else {
            format!("{}/{}", parent_path, name)
        };

        // Fetch attributes
        match self.fetch_attr(&path) {
            Some(attr) => {
                let inode = self
                    .rt
                    .block_on(async { self.inodes.write().await.get_or_create(&path) });
                let file_attr = Self::make_attr(inode, &attr);
                reply.entry(&Self::ATTR_TTL, &file_attr, 0);
            }
            None => {
                reply.error(libc::ENOENT);
            }
        }
    }

    fn getattr(&mut self, _req: &Request<'_>, ino: u64, _fh: Option<u64>, reply: ReplyAttr) {
        let path = {
            let inodes = self.rt.block_on(self.inodes.read());
            match inodes.get_path(ino) {
                Some(p) => p.to_string(),
                None => {
                    reply.error(libc::ENOENT);
                    return;
                }
            }
        };

        match self.fetch_attr(&path) {
            Some(attr) => {
                let file_attr = Self::make_attr(ino, &attr);
                reply.attr(&Self::ATTR_TTL, &file_attr);
            }
            None => {
                reply.error(libc::ENOENT);
            }
        }
    }

    fn setattr(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<TimeOrNow>,
        mtime: Option<TimeOrNow>,
        _ctime: Option<std::time::SystemTime>,
        fh: Option<u64>,
        _crtime: Option<std::time::SystemTime>,
        _chgtime: Option<std::time::SystemTime>,
        _bkuptime: Option<std::time::SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        let path = {
            let inodes = self.rt.block_on(self.inodes.read());
            match inodes.get_path(ino) {
                Some(p) => p.to_string(),
                None => {
                    reply.error(libc::ENOENT);
                    return;
                }
            }
        };

        // Handle size change (truncate)
        if let Some(new_size) = size {
            if self.read_only {
                reply.error(libc::EROFS);
                return;
            }

            if let Err(e) = self.handle_truncate(&path, new_size, fh) {
                reply.error(e);
                return;
            }
        }

        // Handle mtime change - update cache only (no persistence for P2P storage)
        if let Some(mtime_value) = mtime {
            let new_mtime = match mtime_value {
                TimeOrNow::SpecificTime(t) => t
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or_else(|_| chrono::Utc::now().timestamp()),
                TimeOrNow::Now => chrono::Utc::now().timestamp(),
            };

            // Update cached attributes with new mtime
            if let Some(mut attr) = self.fetch_attr(&path) {
                attr.mtime = new_mtime;
                self.cache.put_attr(&path, attr);
            }
        }

        // Return current attributes
        match self.fetch_attr(&path) {
            Some(attr) => {
                let file_attr = Self::make_attr(ino, &attr);
                reply.attr(&Self::ATTR_TTL, &file_attr);
            }
            None => {
                reply.error(libc::ENOENT);
            }
        }
    }

    fn readdir(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        let path = {
            let inodes = self.rt.block_on(self.inodes.read());
            match inodes.get_path(ino) {
                Some(p) => p.to_string(),
                None => {
                    reply.error(libc::ENOENT);
                    return;
                }
            }
        };

        let entries = match self.fetch_dir(&path) {
            Some(e) => e,
            None => {
                reply.error(libc::EIO);
                return;
            }
        };

        // Build full entry list with . and ..
        let mut all_entries: Vec<(u64, FileType, String)> = Vec::new();

        // Add . and ..
        all_entries.push((ino, FileType::Directory, ".".to_string()));
        if ino == InodeTable::ROOT_INODE {
            all_entries.push((ino, FileType::Directory, "..".to_string()));
        } else {
            let parent_path = InodeTable::parent_path(&path);
            let parent_ino = self
                .rt
                .block_on(async { self.inodes.write().await.get_or_create(&parent_path) });
            all_entries.push((parent_ino, FileType::Directory, "..".to_string()));
        }

        // Add directory entries
        for entry in entries {
            let entry_path = if path == "/" {
                format!("/{}", entry.name)
            } else {
                format!("{}/{}", path, entry.name)
            };

            let entry_ino = self
                .rt
                .block_on(async { self.inodes.write().await.get_or_create(&entry_path) });

            let kind = if entry.is_dir {
                FileType::Directory
            } else {
                FileType::RegularFile
            };

            all_entries.push((entry_ino, kind, entry.name));
        }

        // Skip to offset and add entries
        for (i, (ino, kind, name)) in all_entries.into_iter().enumerate().skip(offset as usize) {
            if reply.add(ino, (i + 1) as i64, kind, &name) {
                break;
            }
        }

        reply.ok();
    }

    fn read(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        // Check write buffer first
        {
            let buffers = self.rt.block_on(self.write_buffers.read());
            if let Some(buffer) = buffers.get(&fh) {
                let start = offset as usize;
                let end = (offset as usize + size as usize).min(buffer.data.len());
                if start < buffer.data.len() {
                    reply.data(&buffer.data[start..end]);
                    return;
                }
            }
        }

        let path = {
            let inodes = self.rt.block_on(self.inodes.read());
            match inodes.get_path(ino) {
                Some(p) => p.to_string(),
                None => {
                    reply.error(libc::ENOENT);
                    return;
                }
            }
        };

        match self.fetch_content(&path) {
            Some(content) => {
                let start = offset as usize;
                let end = (offset as usize + size as usize).min(content.data.len());
                if start < content.data.len() {
                    reply.data(&content.data[start..end]);
                } else {
                    reply.data(&[]);
                }
            }
            None => {
                reply.error(libc::EIO);
            }
        }
    }

    fn open(&mut self, _req: &Request<'_>, ino: u64, flags: i32, reply: ReplyOpen) {
        // Check if file exists
        let path = {
            let inodes = self.rt.block_on(self.inodes.read());
            match inodes.get_path(ino) {
                Some(p) => p.to_string(),
                None => {
                    reply.error(libc::ENOENT);
                    return;
                }
            }
        };

        if self.fetch_attr(&path).is_none() {
            reply.error(libc::ENOENT);
            return;
        }

        // Check read-only mode for write access
        let write_flags = libc::O_WRONLY | libc::O_RDWR | libc::O_APPEND | libc::O_TRUNC;
        if self.read_only && (flags & write_flags) != 0 {
            reply.error(libc::EROFS);
            return;
        }

        let fh = self.next_handle();
        reply.opened(fh, 0);
    }

    fn write(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        fh: u64,
        offset: i64,
        data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyWrite,
    ) {
        if self.read_only {
            reply.error(libc::EROFS);
            return;
        }

        let mut buffers = self.rt.block_on(self.write_buffers.write());
        let buffer = buffers.entry(fh).or_insert(WriteBuffer {
            data: Vec::new(),
            dirty: false,
        });

        // Extend buffer if needed
        let end = offset as usize + data.len();
        if buffer.data.len() < end {
            buffer.data.resize(end, 0);
        }

        // Write data
        buffer.data[offset as usize..end].copy_from_slice(data);
        buffer.dirty = true;

        reply.written(data.len() as u32);
    }

    fn flush(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        fh: u64,
        _lock_owner: u64,
        reply: ReplyEmpty,
    ) {
        if self.read_only {
            reply.ok();
            return;
        }

        let path = {
            let inodes = self.rt.block_on(self.inodes.read());
            match inodes.get_path(ino) {
                Some(p) => p.to_string(),
                None => {
                    reply.error(libc::ENOENT);
                    return;
                }
            }
        };

        // Get buffer and flush if dirty
        let buffer_data = {
            let buffers = self.rt.block_on(self.write_buffers.read());
            buffers.get(&fh).filter(|b| b.dirty).map(|b| b.data.clone())
        };

        if let Some(data) = buffer_data {
            let mount = self.mount.clone();
            let fs_path = std::path::Path::new(&path);
            let path_buf = fs_path.to_path_buf();

            let result = self.rt.block_on(async move {
                let mut mount_guard = mount.write().await;
                mount_guard.add(&path_buf, std::io::Cursor::new(data)).await
            });

            match result {
                Ok(_) => {
                    // Mark as clean
                    let mut buffers = self.rt.block_on(self.write_buffers.write());
                    if let Some(buffer) = buffers.get_mut(&fh) {
                        buffer.dirty = false;
                    }
                    // Invalidate cache
                    self.cache.invalidate(&path);

                    // Request save to persist changes
                    if let Some(ref save_tx) = self.save_tx {
                        let mount_id = self.mount_id;
                        let tx = save_tx.clone();
                        self.rt.spawn(async move {
                            if let Err(e) = tx.send(SaveRequest { mount_id }).await {
                                tracing::error!("Failed to send save request: {}", e);
                            }
                        });
                    }

                    reply.ok();
                }
                Err(e) => {
                    tracing::error!("Failed to flush {}: {}", path, e);
                    reply.error(libc::EIO);
                }
            }
        } else {
            reply.ok();
        }
    }

    fn release(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        // Clean up write buffer
        let mut buffers = self.rt.block_on(self.write_buffers.write());
        buffers.remove(&fh);
        reply.ok();
    }

    fn create(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        flags: i32,
        reply: ReplyCreate,
    ) {
        if self.read_only {
            reply.error(libc::EROFS);
            return;
        }

        let name = match name.to_str() {
            Some(n) => n,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let parent_path = {
            let inodes = self.rt.block_on(self.inodes.read());
            match inodes.get_path(parent) {
                Some(p) => p.to_string(),
                None => {
                    reply.error(libc::ENOENT);
                    return;
                }
            }
        };

        let path = if parent_path == "/" {
            format!("/{}", name)
        } else {
            format!("{}/{}", parent_path, name)
        };

        // Create empty file via Mount
        let mount = self.mount.clone();
        let fs_path = std::path::Path::new(&path);
        let path_buf = fs_path.to_path_buf();

        let result = self.rt.block_on(async move {
            let mut mount_guard = mount.write().await;
            mount_guard
                .add(&path_buf, std::io::Cursor::new(Vec::new()))
                .await
        });

        match result {
            Ok(_) => {
                let inode = self
                    .rt
                    .block_on(async { self.inodes.write().await.get_or_create(&path) });

                // Cache the new file's attributes
                let attr = CachedAttr {
                    size: 0,
                    is_dir: false,
                    mime_type: None,
                    mtime: chrono::Utc::now().timestamp(),
                };
                self.cache.put_attr(&path, attr.clone());

                let file_attr = Self::make_attr(inode, &attr);
                let fh = self.next_handle();

                // Initialize write buffer
                let mut buffers = self.rt.block_on(self.write_buffers.write());
                buffers.insert(
                    fh,
                    WriteBuffer {
                        data: Vec::new(),
                        dirty: false,
                    },
                );

                reply.created(&Self::ATTR_TTL, &file_attr, 0, fh, flags as u32);
            }
            Err(e) => {
                tracing::error!("Failed to create {}: {}", path, e);
                reply.error(libc::EIO);
            }
        }
    }

    fn mkdir(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        if self.read_only {
            reply.error(libc::EROFS);
            return;
        }

        let name = match name.to_str() {
            Some(n) => n,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let parent_path = {
            let inodes = self.rt.block_on(self.inodes.read());
            match inodes.get_path(parent) {
                Some(p) => p.to_string(),
                None => {
                    reply.error(libc::ENOENT);
                    return;
                }
            }
        };

        let path = if parent_path == "/" {
            format!("/{}", name)
        } else {
            format!("{}/{}", parent_path, name)
        };

        let mount = self.mount.clone();
        let fs_path = std::path::Path::new(&path);
        let path_buf = fs_path.to_path_buf();

        let result = self.rt.block_on(async move {
            let mut mount_guard = mount.write().await;
            mount_guard.mkdir(&path_buf).await
        });

        match result {
            Ok(_) => {
                let inode = self
                    .rt
                    .block_on(async { self.inodes.write().await.get_or_create(&path) });

                let attr = CachedAttr {
                    size: 0,
                    is_dir: true,
                    mime_type: None,
                    mtime: chrono::Utc::now().timestamp(),
                };
                self.cache.put_attr(&path, attr.clone());

                // Invalidate parent directory cache
                self.cache.invalidate(&parent_path);

                let file_attr = Self::make_attr(inode, &attr);
                reply.entry(&Self::ATTR_TTL, &file_attr, 0);
            }
            Err(e) => {
                tracing::error!("Failed to mkdir {}: {}", path, e);
                reply.error(libc::EIO);
            }
        }
    }

    fn unlink(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        if self.read_only {
            reply.error(libc::EROFS);
            return;
        }

        let name = match name.to_str() {
            Some(n) => n,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let parent_path = {
            let inodes = self.rt.block_on(self.inodes.read());
            match inodes.get_path(parent) {
                Some(p) => p.to_string(),
                None => {
                    reply.error(libc::ENOENT);
                    return;
                }
            }
        };

        let path = if parent_path == "/" {
            format!("/{}", name)
        } else {
            format!("{}/{}", parent_path, name)
        };

        let mount = self.mount.clone();
        let fs_path = std::path::Path::new(&path);
        let path_buf = fs_path.to_path_buf();

        let result = self.rt.block_on(async move {
            let mut mount_guard = mount.write().await;
            mount_guard.rm(&path_buf).await
        });

        match result {
            Ok(_) => {
                // Remove from inode table
                self.rt.block_on(async {
                    self.inodes.write().await.remove_by_path(&path);
                });
                // Invalidate caches
                self.cache.invalidate(&path);
                self.cache.invalidate(&parent_path);

                reply.ok();
            }
            Err(e) => {
                tracing::error!("Failed to unlink {}: {}", path, e);
                reply.error(libc::EIO);
            }
        }
    }

    fn rmdir(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        // rmdir has same implementation as unlink for our purposes
        self.unlink(_req, parent, name, reply);
    }

    fn rename(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        newparent: u64,
        newname: &OsStr,
        _flags: u32,
        reply: ReplyEmpty,
    ) {
        if self.read_only {
            reply.error(libc::EROFS);
            return;
        }

        let name = match name.to_str() {
            Some(n) => n,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let newname = match newname.to_str() {
            Some(n) => n,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let (old_path, new_path, old_parent_path, new_parent_path) = {
            let inodes = self.rt.block_on(self.inodes.read());

            let parent_path = match inodes.get_path(parent) {
                Some(p) => p.to_string(),
                None => {
                    reply.error(libc::ENOENT);
                    return;
                }
            };

            let newparent_path = match inodes.get_path(newparent) {
                Some(p) => p.to_string(),
                None => {
                    reply.error(libc::ENOENT);
                    return;
                }
            };

            let old_path = if parent_path == "/" {
                format!("/{}", name)
            } else {
                format!("{}/{}", parent_path, name)
            };

            let new_path = if newparent_path == "/" {
                format!("/{}", newname)
            } else {
                format!("{}/{}", newparent_path, newname)
            };

            (old_path, new_path, parent_path, newparent_path)
        };

        let mount = self.mount.clone();
        let old_fs_path = std::path::Path::new(&old_path);
        let new_fs_path = std::path::Path::new(&new_path);
        let old_path_buf = old_fs_path.to_path_buf();
        let new_path_buf = new_fs_path.to_path_buf();

        let result = self.rt.block_on(async move {
            let mut mount_guard = mount.write().await;
            mount_guard.mv(&old_path_buf, &new_path_buf).await
        });

        match result {
            Ok(_) => {
                // Update inode table
                self.rt.block_on(async {
                    self.inodes.write().await.rename(&old_path, &new_path);
                });

                // Invalidate caches
                self.cache.invalidate(&old_path);
                self.cache.invalidate(&new_path);
                self.cache.invalidate(&old_parent_path);
                if old_parent_path != new_parent_path {
                    self.cache.invalidate(&new_parent_path);
                }

                reply.ok();
            }
            Err(e) => {
                tracing::error!("Failed to rename {} to {}: {}", old_path, new_path, e);
                reply.error(libc::EIO);
            }
        }
    }

    // Extended attribute stubs - macOS queries these but handles ENOTSUP gracefully
    fn setxattr(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _name: &OsStr,
        _value: &[u8],
        _flags: i32,
        _position: u32,
        reply: ReplyEmpty,
    ) {
        reply.error(libc::ENOTSUP);
    }

    fn getxattr(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _name: &OsStr,
        _size: u32,
        reply: ReplyXattr,
    ) {
        reply.error(libc::ENOTSUP);
    }

    fn listxattr(&mut self, _req: &Request<'_>, _ino: u64, _size: u32, reply: ReplyXattr) {
        reply.error(libc::ENOTSUP);
    }

    fn removexattr(&mut self, _req: &Request<'_>, _ino: u64, _name: &OsStr, reply: ReplyEmpty) {
        reply.error(libc::ENOTSUP);
    }
}
