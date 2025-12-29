//! FUSE filesystem implementation for jax-bucket
//!
//! This module provides a FUSE-based filesystem that mounts a bucket
//! as a local directory, allowing transparent read/write access.

pub mod cache;
pub mod inode_table;
pub mod jax_fs;

pub use cache::{CacheConfig, FileCache};
pub use jax_fs::JaxFs;
