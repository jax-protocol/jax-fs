import { invoke } from "@tauri-apps/api/core";

// Types matching the Rust commands
export interface BucketInfo {
  id: string;
  name: string;
  height: number;
}

export interface FileInfo {
  name: string;
  path: string;
  is_dir: boolean;
  size: number | null;
}

// Bucket operations
export async function listBuckets(): Promise<BucketInfo[]> {
  return invoke("list_buckets");
}

export async function createBucket(name: string): Promise<BucketInfo> {
  return invoke("create_bucket", { name });
}

export async function getBucket(bucketId: string): Promise<BucketInfo> {
  return invoke("get_bucket", { bucketId });
}

// File operations
export async function listFiles(
  bucketId: string,
  path: string
): Promise<FileInfo[]> {
  return invoke("list_files", { bucketId, path });
}

export async function getFile(
  bucketId: string,
  path: string
): Promise<number[]> {
  return invoke("get_file", { bucketId, path });
}

export async function addFile(
  bucketId: string,
  path: string,
  content: number[]
): Promise<void> {
  return invoke("add_file", { bucketId, path, content });
}

export async function deleteFile(
  bucketId: string,
  path: string
): Promise<void> {
  return invoke("delete_file", { bucketId, path });
}

export async function renameFile(
  bucketId: string,
  oldPath: string,
  newPath: string
): Promise<void> {
  return invoke("rename_file", { bucketId, oldPath, newPath });
}

export async function moveFile(
  bucketId: string,
  oldPath: string,
  newPath: string
): Promise<void> {
  return invoke("move_file", { bucketId, oldPath, newPath });
}

export async function createDirectory(
  bucketId: string,
  path: string
): Promise<void> {
  return invoke("create_directory", { bucketId, path });
}
