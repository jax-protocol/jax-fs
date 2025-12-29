//! FUSE filesystem implementation for jax-bucket
//!
//! Maps FUSE operations to daemon HTTP API calls.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, SystemTime};

use base64::Engine;
use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry,
    ReplyOpen, ReplyWrite, Request,
};
use uuid::Uuid;

use super::cache::{CacheConfig, FileCache};
use super::inode_table::InodeTable;
use crate::daemon::http_server::api::client::ApiClient;
use crate::daemon::http_server::api::v0::bucket::{
    cat::{CatRequest, CatResponse},
    ls::{LsRequest, LsResponse},
};

/// TTL for filesystem attributes (1 second for now)
const TTL: Duration = Duration::from_secs(1);

/// Default block size
const BLOCK_SIZE: u32 = 512;

/// FUSE filesystem backed by jax-bucket daemon
pub struct JaxFs {
    /// Tokio runtime handle for async operations
    rt: tokio::runtime::Handle,

    /// HTTP client for daemon API
    client: ApiClient,

    /// Bucket ID being mounted
    bucket_id: Uuid,

    /// Inode to path mapping
    inodes: RwLock<InodeTable>,

    /// Write buffers keyed by file handle
    write_buffers: RwLock<HashMap<u64, WriteBuffer>>,

    /// Next file handle to assign
    next_fh: AtomicU64,

    /// Cache of file sizes (path -> size)
    size_cache: RwLock<HashMap<PathBuf, u64>>,

    /// LRU cache for file content
    cache: FileCache,
}

/// Buffer for accumulating writes before flush
struct WriteBuffer {
    path: PathBuf,
    data: Vec<u8>,
    /// Whether we loaded the original file content (for append/modify operations)
    original_loaded: bool,
    /// Track if any writes happened (to avoid re-uploading unchanged files)
    dirty: bool,
}

impl JaxFs {
    /// Create a new FUSE filesystem with default cache config
    pub fn new(rt: tokio::runtime::Handle, client: ApiClient, bucket_id: Uuid) -> Self {
        Self::with_cache_config(rt, client, bucket_id, CacheConfig::default())
    }

    /// Create a new FUSE filesystem with custom cache config
    pub fn with_cache_config(
        rt: tokio::runtime::Handle,
        client: ApiClient,
        bucket_id: Uuid,
        cache_config: CacheConfig,
    ) -> Self {
        Self {
            rt,
            client,
            bucket_id,
            inodes: RwLock::new(InodeTable::new()),
            write_buffers: RwLock::new(HashMap::new()),
            next_fh: AtomicU64::new(1),
            size_cache: RwLock::new(HashMap::new()),
            cache: FileCache::new(cache_config),
        }
    }

    /// Invalidate the entire cache (called on remote sync)
    pub fn invalidate_cache(&self) {
        self.cache.invalidate_all();
    }

    /// Make a file attribute structure
    fn make_attr(&self, ino: u64, is_dir: bool, size: u64) -> FileAttr {
        let now = SystemTime::now();
        FileAttr {
            ino,
            size,
            blocks: (size + BLOCK_SIZE as u64 - 1) / BLOCK_SIZE as u64,
            atime: now,
            mtime: now,
            ctime: now,
            crtime: now,
            kind: if is_dir { FileType::Directory } else { FileType::RegularFile },
            perm: if is_dir { 0o755 } else { 0o644 },
            nlink: 1,
            uid: unsafe { libc::getuid() },
            gid: unsafe { libc::getgid() },
            rdev: 0,
            blksize: BLOCK_SIZE,
            flags: 0,
        }
    }

    /// Call the ls API endpoint
    fn api_ls(&self, path: &str) -> Result<LsResponse, String> {
        let mut client = self.client.clone();
        let req = LsRequest {
            bucket_id: self.bucket_id,
            path: Some(path.to_string()),
            deep: Some(false),
        };

        self.rt
            .block_on(async { client.call(req).await })
            .map_err(|e| e.to_string())
    }

    /// Call the cat API endpoint
    fn api_cat(&self, path: &str) -> Result<Vec<u8>, String> {
        let mut client = self.client.clone();
        let req = CatRequest {
            bucket_id: self.bucket_id,
            path: path.to_string(),
            at: None,
            download: None,
        };

        let response: CatResponse = self.rt
            .block_on(async { client.call(req).await })
            .map_err(|e| e.to_string())?;

        // Decode base64 content
        base64::engine::general_purpose::STANDARD
            .decode(&response.content)
            .map_err(|e| e.to_string())
    }

    /// Call the mkdir API endpoint (direct HTTP, no ApiRequest impl)
    fn api_mkdir(&self, path: &str) -> Result<(), String> {
        let client = self.client.clone();
        let bucket_id = self.bucket_id;

        self.rt.block_on(async {
            let url = client.base_url().join("/api/v0/bucket/mkdir").unwrap();

            #[derive(serde::Serialize)]
            struct MkdirRequest {
                bucket_id: Uuid,
                path: String,
            }

            let response = client
                .http_client()
                .post(url)
                .json(&MkdirRequest {
                    bucket_id,
                    path: path.to_string(),
                })
                .send()
                .await
                .map_err(|e| e.to_string())?;

            if response.status().is_success() {
                Ok(())
            } else {
                Err(format!("mkdir failed: {}", response.status()))
            }
        })
    }

    /// Call the delete API endpoint (direct HTTP, no ApiRequest impl)
    fn api_delete(&self, path: &str) -> Result<(), String> {
        let client = self.client.clone();
        let bucket_id = self.bucket_id;

        self.rt.block_on(async {
            let url = client.base_url().join("/api/v0/bucket/delete").unwrap();

            #[derive(serde::Serialize)]
            struct DeleteRequest {
                bucket_id: Uuid,
                path: String,
            }

            let response = client
                .http_client()
                .post(url)
                .json(&DeleteRequest {
                    bucket_id,
                    path: path.to_string(),
                })
                .send()
                .await
                .map_err(|e| e.to_string())?;

            if response.status().is_success() {
                Ok(())
            } else {
                Err(format!("delete failed: {}", response.status()))
            }
        })
    }

    /// Call the mv API endpoint (direct HTTP, no ApiRequest impl)
    fn api_mv(&self, from: &str, to: &str) -> Result<(), String> {
        let client = self.client.clone();
        let bucket_id = self.bucket_id;

        self.rt.block_on(async {
            let url = client.base_url().join("/api/v0/bucket/mv").unwrap();

            #[derive(serde::Serialize)]
            struct MvRequest {
                bucket_id: Uuid,
                source_path: String,
                dest_path: String,
            }

            let response = client
                .http_client()
                .post(url)
                .json(&MvRequest {
                    bucket_id,
                    source_path: from.to_string(),
                    dest_path: to.to_string(),
                })
                .send()
                .await
                .map_err(|e| e.to_string())?;

            if response.status().is_success() {
                Ok(())
            } else {
                Err(format!("mv failed: {}", response.status()))
            }
        })
    }

    /// Upload a file via multipart form (add endpoint)
    fn api_add(&self, path: &str, data: Vec<u8>) -> Result<(), String> {
        let client = self.client.clone();
        let bucket_id = self.bucket_id;

        self.rt.block_on(async {
            let url = client.base_url().join("/api/v0/bucket/add").unwrap();

            let form = reqwest::multipart::Form::new()
                .text("bucket_id", bucket_id.to_string())
                .text("mount_path", path.to_string())
                .part(
                    "file",
                    reqwest::multipart::Part::bytes(data)
                        .file_name("file")
                        .mime_str("application/octet-stream")
                        .unwrap(),
                );

            let response = client
                .http_client()
                .post(url)
                .multipart(form)
                .send()
                .await
                .map_err(|e| e.to_string())?;

            if response.status().is_success() {
                Ok(())
            } else {
                Err(format!("Add failed: {}", response.status()))
            }
        })
    }
}

impl Filesystem for JaxFs {
    /// Look up a directory entry by name
    fn lookup(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => return reply.error(libc::ENOENT),
        };

        // Get parent path
        let parent_path = {
            let inodes = self.inodes.read().unwrap();
            match inodes.get_path(parent) {
                Some(p) => p.to_path_buf(),
                None => return reply.error(libc::ENOENT),
            }
        };

        // Construct child path
        let child_path = parent_path.join(name_str);
        let child_path_str = child_path.to_string_lossy().to_string();

        // Check if it exists by listing parent
        let parent_path_str = parent_path.to_string_lossy().to_string();
        match self.api_ls(&parent_path_str) {
            Ok(response) => {
                for item in response.items {
                    if item.path == child_path_str || item.name == name_str {
                        let mut inodes = self.inodes.write().unwrap();
                        let child_path_buf = PathBuf::from(&item.path);
                        let ino = inodes.get_or_create(&child_path_buf);

                        // Cache size for files
                        if !item.is_dir {
                            // We don't have size from ls, so we'll get it on read
                            // For now, use 0 and update on getattr
                        }

                        let attr = self.make_attr(ino, item.is_dir, 0);
                        return reply.entry(&TTL, &attr, 0);
                    }
                }
                reply.error(libc::ENOENT)
            }
            Err(_) => reply.error(libc::EIO),
        }
    }

    /// Get file attributes
    fn getattr(&mut self, _req: &Request<'_>, ino: u64, _fh: Option<u64>, reply: ReplyAttr) {
        if ino == 1 {
            // Root directory
            let attr = self.make_attr(1, true, 0);
            return reply.attr(&TTL, &attr);
        }

        let path = {
            let inodes = self.inodes.read().unwrap();
            match inodes.get_path(ino) {
                Some(p) => p.to_path_buf(),
                None => return reply.error(libc::ENOENT),
            }
        };

        // Get parent to list and find this item
        let parent_path = path.parent().unwrap_or(&PathBuf::from("/")).to_path_buf();
        let parent_path_str = parent_path.to_string_lossy().to_string();
        let path_str = path.to_string_lossy().to_string();

        match self.api_ls(&parent_path_str) {
            Ok(response) => {
                for item in response.items {
                    if item.path == path_str {
                        let size = if item.is_dir {
                            0
                        } else {
                            // Get actual size by reading the file
                            self.size_cache
                                .read()
                                .unwrap()
                                .get(&path)
                                .copied()
                                .unwrap_or(0)
                        };

                        let attr = self.make_attr(ino, item.is_dir, size);
                        return reply.attr(&TTL, &attr);
                    }
                }
                reply.error(libc::ENOENT)
            }
            Err(_) => reply.error(libc::EIO),
        }
    }

    /// Read directory contents
    fn readdir(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        let path = {
            let inodes = self.inodes.read().unwrap();
            match inodes.get_path(ino) {
                Some(p) => p.to_path_buf(),
                None => return reply.error(libc::ENOENT),
            }
        };

        let path_str = path.to_string_lossy().to_string();

        match self.api_ls(&path_str) {
            Ok(response) => {
                let mut entries = vec![
                    (ino, FileType::Directory, ".".to_string()),
                    (ino, FileType::Directory, "..".to_string()),
                ];

                for item in response.items {
                    let item_path = PathBuf::from(&item.path);
                    let mut inodes = self.inodes.write().unwrap();
                    let item_ino = inodes.get_or_create(&item_path);
                    let kind = if item.is_dir {
                        FileType::Directory
                    } else {
                        FileType::RegularFile
                    };
                    entries.push((item_ino, kind, item.name));
                }

                for (i, (ino, kind, name)) in entries.iter().enumerate().skip(offset as usize) {
                    if reply.add(*ino, (i + 1) as i64, *kind, name) {
                        break;
                    }
                }

                reply.ok()
            }
            Err(_) => reply.error(libc::EIO),
        }
    }

    /// Read file data
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
        let path = {
            let inodes = self.inodes.read().unwrap();
            match inodes.get_path(ino) {
                Some(p) => p.to_path_buf(),
                None => return reply.error(libc::ENOENT),
            }
        };

        // If we have an active write buffer for this fh, read from it (O_RDWR support)
        {
            let buffers = self.write_buffers.read().unwrap();
            if let Some(buffer) = buffers.get(&fh) {
                let start = offset as usize;
                if start >= buffer.data.len() {
                    return reply.data(&[]);
                }
                let end = (start + size as usize).min(buffer.data.len());
                return reply.data(&buffer.data[start..end]);
            }
        }

        // Check cache first
        if let Some(data) = self.cache.get(&path) {
            let start = offset as usize;
            if start >= data.len() {
                return reply.data(&[]);
            }
            let end = (start + size as usize).min(data.len());
            return reply.data(&data[start..end]);
        }

        // Cache miss - fetch from daemon
        let path_str = path.to_string_lossy().to_string();

        match self.api_cat(&path_str) {
            Ok(data) => {
                // Cache the content
                self.cache.put(path.clone(), data.clone());

                // Also update size cache
                {
                    let mut size_cache = self.size_cache.write().unwrap();
                    size_cache.insert(path, data.len() as u64);
                }

                let start = offset as usize;
                if start >= data.len() {
                    return reply.data(&[]);
                }
                let end = (start + size as usize).min(data.len());
                reply.data(&data[start..end])
            }
            Err(_) => reply.error(libc::EIO),
        }
    }

    /// Open a file (for read or write)
    fn open(&mut self, _req: &Request<'_>, ino: u64, flags: i32, reply: ReplyOpen) {
        let fh = self.next_fh.fetch_add(1, Ordering::SeqCst);

        // Check if opening for write
        let write_mode = (flags & libc::O_WRONLY != 0) || (flags & libc::O_RDWR != 0);
        let truncate = (flags & libc::O_TRUNC != 0);
        let append = (flags & libc::O_APPEND != 0);

        if write_mode || truncate {
            let path = {
                let inodes = self.inodes.read().unwrap();
                match inodes.get_path(ino) {
                    Some(p) => p.to_path_buf(),
                    None => return reply.error(libc::ENOENT),
                }
            };

            // If not truncating, we need to load existing content for modify-in-place
            let (initial_data, original_loaded) = if truncate {
                (Vec::new(), false)
            } else {
                // Try to load existing file content
                let path_str = path.to_string_lossy().to_string();
                match self.api_cat(&path_str) {
                    Ok(data) => (data, true),
                    Err(_) => (Vec::new(), false), // File might be new or empty
                }
            };

            let mut buffers = self.write_buffers.write().unwrap();
            buffers.insert(
                fh,
                WriteBuffer {
                    path,
                    data: initial_data,
                    original_loaded,
                    dirty: false,
                },
            );
        }

        reply.opened(fh, 0)
    }

    /// Write data to a file
    fn write(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        fh: u64,
        offset: i64,
        data: &[u8],
        _write_flags: u32,
        flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyWrite,
    ) {
        let mut buffers = self.write_buffers.write().unwrap();

        if let Some(buffer) = buffers.get_mut(&fh) {
            // Handle O_APPEND: always write at end
            let write_offset = if flags & libc::O_APPEND != 0 {
                buffer.data.len()
            } else {
                offset as usize
            };

            // Extend buffer if needed (fill with zeros)
            if buffer.data.len() < write_offset {
                buffer.data.resize(write_offset, 0);
            }

            // Write data at offset
            let end = write_offset + data.len();
            if buffer.data.len() < end {
                buffer.data.resize(end, 0);
            }
            buffer.data[write_offset..end].copy_from_slice(data);

            // Mark as dirty
            buffer.dirty = true;

            reply.written(data.len() as u32)
        } else {
            reply.error(libc::EBADF)
        }
    }

    /// Flush file data (called before close)
    fn flush(&mut self, _req: &Request<'_>, _ino: u64, fh: u64, _lock_owner: u64, reply: ReplyEmpty) {
        // Check if we have buffered writes
        let buffer_info = {
            let buffers = self.write_buffers.read().unwrap();
            buffers
                .get(&fh)
                .map(|b| (b.path.clone(), b.data.clone(), b.dirty))
        };

        if let Some((path, data, dirty)) = buffer_info {
            // Only upload if the buffer was modified
            if dirty {
                let path_str = path.to_string_lossy().to_string();
                if let Err(_) = self.api_add(&path_str, data.clone()) {
                    return reply.error(libc::EIO);
                }

                // Invalidate cache for this path (we just wrote to it)
                self.cache.invalidate(&path);

                // Update size cache
                {
                    let mut size_cache = self.size_cache.write().unwrap();
                    size_cache.insert(path.clone(), data.len() as u64);
                }

                // Update content cache with new data
                self.cache.put(path, data);
            }
        }

        reply.ok()
    }

    /// Release (close) a file
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
        // Remove write buffer
        let mut buffers = self.write_buffers.write().unwrap();
        buffers.remove(&fh);

        reply.ok()
    }

    /// Create a file
    fn create(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        flags: i32,
        reply: fuser::ReplyCreate,
    ) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => return reply.error(libc::EINVAL),
        };

        let parent_path = {
            let inodes = self.inodes.read().unwrap();
            match inodes.get_path(parent) {
                Some(p) => p.to_path_buf(),
                None => return reply.error(libc::ENOENT),
            }
        };

        let file_path = parent_path.join(name_str);
        let file_path_str = file_path.to_string_lossy().to_string();

        // Create empty file
        if let Err(_) = self.api_add(&file_path_str, Vec::new()) {
            return reply.error(libc::EIO);
        }

        // Create inode
        let ino = {
            let mut inodes = self.inodes.write().unwrap();
            inodes.get_or_create(&file_path)
        };

        let fh = self.next_fh.fetch_add(1, Ordering::SeqCst);

        // Set up write buffer if needed
        let write_mode = (flags & libc::O_WRONLY != 0) || (flags & libc::O_RDWR != 0);
        if write_mode {
            let mut buffers = self.write_buffers.write().unwrap();
            buffers.insert(
                fh,
                WriteBuffer {
                    path: file_path,
                    data: Vec::new(),
                    original_loaded: false,
                    dirty: false,
                },
            );
        }

        let attr = self.make_attr(ino, false, 0);
        reply.created(&TTL, &attr, 0, fh, 0)
    }

    /// Create a directory
    fn mkdir(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => return reply.error(libc::EINVAL),
        };

        let parent_path = {
            let inodes = self.inodes.read().unwrap();
            match inodes.get_path(parent) {
                Some(p) => p.to_path_buf(),
                None => return reply.error(libc::ENOENT),
            }
        };

        let dir_path = parent_path.join(name_str);
        let dir_path_str = dir_path.to_string_lossy().to_string();

        if let Err(_) = self.api_mkdir(&dir_path_str) {
            return reply.error(libc::EIO);
        }

        let ino = {
            let mut inodes = self.inodes.write().unwrap();
            inodes.get_or_create(&dir_path)
        };

        let attr = self.make_attr(ino, true, 0);
        reply.entry(&TTL, &attr, 0)
    }

    /// Remove a file
    fn unlink(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => return reply.error(libc::EINVAL),
        };

        let parent_path = {
            let inodes = self.inodes.read().unwrap();
            match inodes.get_path(parent) {
                Some(p) => p.to_path_buf(),
                None => return reply.error(libc::ENOENT),
            }
        };

        let file_path = parent_path.join(name_str);
        let file_path_str = file_path.to_string_lossy().to_string();

        if let Err(_) = self.api_delete(&file_path_str) {
            return reply.error(libc::EIO);
        }

        // Invalidate content cache
        self.cache.invalidate(&file_path);

        // Remove from inode table
        {
            let mut inodes = self.inodes.write().unwrap();
            if let Some(ino) = inodes.get_inode(&file_path) {
                inodes.remove(ino);
            }
        }

        // Remove from size cache
        {
            let mut size_cache = self.size_cache.write().unwrap();
            size_cache.remove(&file_path);
        }

        reply.ok()
    }

    /// Remove a directory
    fn rmdir(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        // Same as unlink for our implementation
        self.unlink(_req, parent, name, reply)
    }

    /// Rename a file or directory
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
        let name_str = match name.to_str() {
            Some(s) => s,
            None => return reply.error(libc::EINVAL),
        };
        let newname_str = match newname.to_str() {
            Some(s) => s,
            None => return reply.error(libc::EINVAL),
        };

        let (from_path, to_path) = {
            let inodes = self.inodes.read().unwrap();
            let parent_path = match inodes.get_path(parent) {
                Some(p) => p.to_path_buf(),
                None => return reply.error(libc::ENOENT),
            };
            let newparent_path = match inodes.get_path(newparent) {
                Some(p) => p.to_path_buf(),
                None => return reply.error(libc::ENOENT),
            };

            (parent_path.join(name_str), newparent_path.join(newname_str))
        };

        let from_path_str = from_path.to_string_lossy().to_string();
        let to_path_str = to_path.to_string_lossy().to_string();

        if let Err(_) = self.api_mv(&from_path_str, &to_path_str) {
            return reply.error(libc::EIO);
        }

        // Invalidate cache for both old and new paths
        self.cache.invalidate(&from_path);
        self.cache.invalidate(&to_path);

        // Update inode table
        {
            let mut inodes = self.inodes.write().unwrap();
            inodes.rename(&from_path, &to_path);
        }

        // Update size cache
        {
            let mut size_cache = self.size_cache.write().unwrap();
            if let Some(size) = size_cache.remove(&from_path) {
                size_cache.insert(to_path, size);
            }
        }

        reply.ok()
    }

    /// Set file attributes (minimal implementation)
    fn setattr(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        _size: Option<u64>,
        _atime: Option<fuser::TimeOrNow>,
        _mtime: Option<fuser::TimeOrNow>,
        _ctime: Option<SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        // For truncate (size = 0), we handle it
        if let Some(0) = _size {
            let path = {
                let inodes = self.inodes.read().unwrap();
                match inodes.get_path(ino) {
                    Some(p) => p.to_path_buf(),
                    None => return reply.error(libc::ENOENT),
                }
            };

            let path_str = path.to_string_lossy().to_string();

            // Create empty file (truncate)
            if let Err(_) = self.api_add(&path_str, Vec::new()) {
                return reply.error(libc::EIO);
            }

            // Invalidate content cache
            self.cache.invalidate(&path);

            // Update size cache
            {
                let mut size_cache = self.size_cache.write().unwrap();
                size_cache.insert(path, 0);
            }
        }

        // Return current attributes
        self.getattr(_req, ino, _fh, reply)
    }
}
