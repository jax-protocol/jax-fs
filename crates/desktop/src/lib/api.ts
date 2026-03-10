/**
 * Tauri IPC API wrapper functions
 */
import { invoke } from '@tauri-apps/api/core';

// Types
export interface BucketInfo {
  bucket_id: string;
  name: string;
  link_hash: string;
  height: number;
  created_at: string;
}

export interface FileEntry {
  path: string;
  name: string;
  is_dir: boolean;
  mime_type: string;
  link_hash: string;
}

export interface CatResult {
  content: number[];
  mime_type: string;
  size: number;
}

export interface DaemonStatus {
  running: boolean;
  api_port: number;
  gateway_port: number;
  node_id: string | null;
}

export interface HistoryEntry {
  link_hash: string;
  height: number;
  published: boolean;
  created_at: string;
}

export interface ShareInfo {
  public_key: string;
  role: string;
  is_self: boolean;
}

// Bucket operations
export async function listBuckets(): Promise<BucketInfo[]> {
  return invoke('list_buckets');
}

export async function createBucket(name: string): Promise<BucketInfo> {
  return invoke('create_bucket', { name });
}

export async function deleteBucket(bucketId: string): Promise<void> {
  return invoke('delete_bucket', { bucketId });
}

export async function ls(bucketId: string, path: string): Promise<FileEntry[]> {
  return invoke('ls', { bucketId, path });
}

export async function cat(bucketId: string, path: string): Promise<CatResult> {
  return invoke('cat', { bucketId, path });
}

export async function addFile(bucketId: string, path: string, data: number[]): Promise<void> {
  return invoke('add_file', { bucketId, path, data });
}

export async function updateFile(bucketId: string, path: string, data: number[]): Promise<void> {
  return invoke('update_file', { bucketId, path, data });
}

export async function renamePath(bucketId: string, oldPath: string, newPath: string): Promise<void> {
  return invoke('rename_path', { bucketId, oldPath, newPath });
}

export async function movePath(bucketId: string, sourcePath: string, destPath: string): Promise<void> {
  return invoke('move_path', { bucketId, sourcePath, destPath });
}

export async function shareBucket(bucketId: string, peerPublicKey: string, role: string): Promise<void> {
  return invoke('share_bucket', { bucketId, peerPublicKey, role });
}

export async function isPublished(bucketId: string): Promise<boolean> {
  return invoke('is_published', { bucketId });
}

export async function publishBucket(bucketId: string): Promise<void> {
  return invoke('publish_bucket', { bucketId });
}

export async function unpublishBucket(bucketId: string): Promise<void> {
  return invoke('unpublish_bucket', { bucketId });
}

export async function pingPeer(bucketId: string, peerPublicKey: string): Promise<string> {
  return invoke('ping_peer', { bucketId, peerPublicKey });
}

export async function uploadNativeFiles(bucketId: string, mountPath: string, filePaths: string[]): Promise<void> {
  return invoke('upload_native_files', { bucketId, mountPath, filePaths });
}

export async function exportFile(bucketId: string, path: string, destPath: string): Promise<void> {
  return invoke('export_file', { bucketId, path, destPath });
}

export async function mkdir(bucketId: string, path: string): Promise<void> {
  return invoke('mkdir', { bucketId, path });
}

export async function deletePath(bucketId: string, path: string): Promise<void> {
  return invoke('delete_path', { bucketId, path });
}

export async function getBucketShares(bucketId: string): Promise<ShareInfo[]> {
  return invoke('get_bucket_shares', { bucketId });
}

export async function removeShare(bucketId: string, peerPublicKey: string): Promise<void> {
  return invoke('remove_share', { bucketId, peerPublicKey });
}

// History operations
export async function getHistory(bucketId: string, page?: number): Promise<HistoryEntry[]> {
  return invoke('get_history', { bucketId, page: page ?? null });
}

export async function lsAtVersion(bucketId: string, linkHash: string, path: string): Promise<FileEntry[]> {
  return invoke('ls_at_version', { bucketId, linkHash, path });
}

export async function catAtVersion(bucketId: string, linkHash: string, path: string): Promise<CatResult> {
  return invoke('cat_at_version', { bucketId, linkHash, path });
}

// Daemon operations
export async function getStatus(): Promise<DaemonStatus> {
  return invoke('get_status');
}

export async function getIdentity(): Promise<string> {
  return invoke('get_identity');
}

export interface ConfigInfo {
  jax_dir: string;
  db_path: string;
  config_path: string;
  blob_store: string;
}

export async function getConfigInfo(): Promise<ConfigInfo> {
  return invoke('get_config_info');
}

// Mount types
export interface MountInfo {
  mount_id: string;
  bucket_id: string;
  mount_point: string;
  enabled: boolean;
  auto_mount: boolean;
  read_only: boolean;
  cache_size_mb: number;
  cache_ttl_secs: number;
  status: string;
  error_message: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateMountRequest {
  bucket_id: string;
  mount_point: string;
  auto_mount?: boolean;
  read_only?: boolean;
  cache_size_mb?: number;
  cache_ttl_secs?: number;
}

export interface UpdateMountRequest {
  mount_point?: string;
  enabled?: boolean;
  auto_mount?: boolean;
  read_only?: boolean;
  cache_size_mb?: number;
  cache_ttl_secs?: number;
}

// Mount operations
export async function listMounts(): Promise<MountInfo[]> {
  return invoke('list_mounts');
}

export async function createMount(request: CreateMountRequest): Promise<MountInfo> {
  return invoke('create_mount', { request });
}

export async function getMount(mountId: string): Promise<MountInfo> {
  return invoke('get_mount', { mountId });
}

export async function updateMount(mountId: string, request: UpdateMountRequest): Promise<MountInfo> {
  return invoke('update_mount', { mountId, request });
}

export async function deleteMount(mountId: string): Promise<boolean> {
  return invoke('delete_mount', { mountId });
}

export async function startMount(mountId: string): Promise<boolean> {
  return invoke('start_mount', { mountId });
}

export async function stopMount(mountId: string): Promise<boolean> {
  return invoke('stop_mount', { mountId });
}

export async function isFuseAvailable(): Promise<boolean> {
  return invoke('is_fuse_available');
}

// Simplified mount API (auto mount point selection)
export async function mountBucket(bucketId: string): Promise<MountInfo> {
  return invoke('mount_bucket', { bucketId });
}

export async function unmountBucket(bucketId: string): Promise<boolean> {
  return invoke('unmount_bucket', { bucketId });
}

export async function isBucketMounted(bucketId: string): Promise<MountInfo | null> {
  return invoke('is_bucket_mounted', { bucketId });
}

// Log types
export interface LogFileInfo {
  filename: string;
  size: number;
  modified: string;
}

export interface LogFileContents {
  lines: string[];
  total_lines: number;
  has_more: boolean;
}

// Log operations
export async function listLogFiles(): Promise<LogFileInfo[]> {
  return invoke('list_log_files');
}

export async function readLogFile(filename: string, offset?: number, limit?: number): Promise<LogFileContents> {
  return invoke('read_log_file', { filename, offset: offset ?? null, limit: limit ?? null });
}

export async function tailLogFile(lines?: number): Promise<string[]> {
  return invoke('tail_log_file', { lines: lines ?? null });
}

export async function subscribeLogs(): Promise<void> {
  return invoke('subscribe_logs');
}

export async function unsubscribeLogs(): Promise<void> {
  return invoke('unsubscribe_logs');
}
