//! Integration tests for conflict resolution on divergent mounts

mod common;

use std::io::Cursor;
use std::path::PathBuf;

use ::common::mount::{merge_logs, ConflictFile, OpType, Resolution};

/// Alice and Bob both create "notes.txt" with different content.
/// Alice's stays at the original path, Bob's is renamed to notes@<hash>.txt.
#[tokio::test]
async fn test_single_file_conflict() {
    let (mut alice, blobs, _, _temp) = common::setup_test_env().await;
    let (mut bob, _bob_key) = common::fork_mount(&mut alice, &blobs).await;

    // Both create notes.txt offline
    alice
        .add(
            &PathBuf::from("/notes.txt"),
            Cursor::new(b"Alice's notes".to_vec()),
        )
        .await
        .unwrap();
    bob.add(
        &PathBuf::from("/notes.txt"),
        Cursor::new(b"Bob's notes".to_vec()),
    )
    .await
    .unwrap();

    // Save both mounts to persist their ops_logs
    alice.save(&blobs, None).await.unwrap();
    bob.save(&blobs, None).await.unwrap();

    // Merge Bob's changes into Alice using the new API
    let resolver = ConflictFile::new();
    let (result, _new_link) = alice.merge_from(&bob, &resolver, &blobs).await.unwrap();

    // One conflict: notes.txt
    assert_eq!(result.total_conflicts(), 1);
    let resolved = &result.conflicts_resolved[0];
    match &resolved.resolution {
        Resolution::RenameIncoming { new_path } => {
            assert!(new_path.to_string_lossy().starts_with("notes.txt@"));
        }
        other => panic!("Expected RenameIncoming, got {:?}", other),
    }

    // Verify merge was successful
    // Either operations were added or conflicts were resolved
    assert!(result.operations_added > 0 || result.total_conflicts() > 0);
}

/// Test the low-level merge_logs function for direct ops_log manipulation
#[tokio::test]
async fn test_merge_logs_low_level() {
    let (mut alice, blobs, _, _temp) = common::setup_test_env().await;
    let (mut bob, bob_key) = common::fork_mount(&mut alice, &blobs).await;

    // Both create notes.txt offline
    alice
        .add(
            &PathBuf::from("/notes.txt"),
            Cursor::new(b"Alice's notes".to_vec()),
        )
        .await
        .unwrap();
    bob.add(
        &PathBuf::from("/notes.txt"),
        Cursor::new(b"Bob's notes".to_vec()),
    )
    .await
    .unwrap();

    // Merge using the low-level API (without saving)
    let alice_log = alice.inner().await.ops_log().clone();
    let bob_log = bob.inner().await.ops_log().clone();
    let resolver = ConflictFile::new();
    let (merged, results) = merge_logs(&[&alice_log, &bob_log], &resolver, &bob_key.public());

    // One conflict: notes.txt
    assert_eq!(results[0].total_conflicts(), 1);
    let resolved = &results[0].conflicts_resolved[0];
    match &resolved.resolution {
        Resolution::RenameIncoming { new_path } => {
            assert!(new_path.to_string_lossy().starts_with("notes.txt@"));
        }
        other => panic!("Expected RenameIncoming, got {:?}", other),
    }

    // Both files exist
    let final_state = merged.resolve_all();
    assert!(final_state.contains_key(&PathBuf::from("notes.txt")));
    assert_eq!(
        merged
            .ops_in_order()
            .filter(|op| matches!(op.op_type, OpType::Add))
            .count(),
        2
    );
}

/// The realistic scenario: peers diverge for MULTIPLE save cycles before syncing.
///
/// This simulates the DAG-like manifest chain where a peer discovers they're
/// behind a longer canonical chain and need to merge all accumulated operations.
///
/// Timeline:
/// ```text
/// v0 (common ancestor - empty bucket)
///  │
///  ├── Alice's chain (local):
///  │   v1: adds README.md, creates src/
///  │   v2: adds src/main.rs, src/utils.rs
///  │   v3: adds config.toml
///  │
///  └── Bob's chain (remote canonical):
///      v1': adds CONTRIBUTING.md, creates src/
///      v2': adds src/lib.rs
///      v3': adds config.toml (CONFLICT!), adds .gitignore
/// ```
///
/// When Alice syncs, she merges Bob's chain into hers. The only conflict
/// is `config.toml` - everything else merges cleanly because the paths
/// don't overlap.
#[tokio::test]
async fn test_multi_version_divergence() {
    let (mut alice, blobs, _, _temp) = common::setup_test_env().await;
    let (mut bob, _bob_key) = common::fork_mount(&mut alice, &blobs).await;

    // -------------------------------------------------------------------------
    // Alice's chain: 3 versions of changes
    // -------------------------------------------------------------------------

    // v1: Alice sets up the project
    alice
        .add(
            &PathBuf::from("/README.md"),
            Cursor::new(b"Alice README".to_vec()),
        )
        .await
        .unwrap();
    alice.mkdir(&PathBuf::from("/src")).await.unwrap();
    alice.save(&blobs, None).await.unwrap();

    // v2: Alice adds source files
    alice
        .add(
            &PathBuf::from("/src/main.rs"),
            Cursor::new(b"fn main() {}".to_vec()),
        )
        .await
        .unwrap();
    alice
        .add(
            &PathBuf::from("/src/utils.rs"),
            Cursor::new(b"pub fn help() {}".to_vec()),
        )
        .await
        .unwrap();
    alice.save(&blobs, None).await.unwrap();

    // v3: Alice adds config
    alice
        .add(
            &PathBuf::from("/config.toml"),
            Cursor::new(b"[alice]".to_vec()),
        )
        .await
        .unwrap();
    alice.save(&blobs, None).await.unwrap();

    // -------------------------------------------------------------------------
    // Bob's chain: 3 versions, diverging from the same v0
    // -------------------------------------------------------------------------

    // v1': Bob sets up differently
    bob.add(
        &PathBuf::from("/CONTRIBUTING.md"),
        Cursor::new(b"Bob contrib".to_vec()),
    )
    .await
    .unwrap();
    bob.mkdir(&PathBuf::from("/src")).await.unwrap();
    bob.save(&blobs, None).await.unwrap();

    // v2': Bob adds his source files
    bob.add(
        &PathBuf::from("/src/lib.rs"),
        Cursor::new(b"pub mod tests;".to_vec()),
    )
    .await
    .unwrap();
    bob.save(&blobs, None).await.unwrap();

    // v3': Bob adds config (CONFLICTS with Alice!) and .gitignore
    bob.add(
        &PathBuf::from("/config.toml"),
        Cursor::new(b"[bob]".to_vec()),
    )
    .await
    .unwrap();
    bob.add(
        &PathBuf::from("/.gitignore"),
        Cursor::new(b"target/".to_vec()),
    )
    .await
    .unwrap();
    bob.save(&blobs, None).await.unwrap();

    // -------------------------------------------------------------------------
    // Merge: Alice discovers Bob's chain and merges it using new API
    // -------------------------------------------------------------------------

    // First, verify the common ancestor is found correctly
    let ancestor = alice.find_common_ancestor(&bob, &blobs).await.unwrap();
    assert!(ancestor.is_some(), "Should find common ancestor");

    // Now merge Bob's changes into Alice
    let resolver = ConflictFile::new();
    let (result, _new_link) = alice.merge_from(&bob, &resolver, &blobs).await.unwrap();

    // Only config.toml conflicts
    let config_conflicts: Vec<_> = result
        .conflicts_resolved
        .iter()
        .filter(|c| c.conflict.path.to_string_lossy().contains("config"))
        .collect();
    assert_eq!(config_conflicts.len(), 1);

    // The conflict resolution should produce a config@<hash>.toml file
    // where <hash> is derived from Bob's content link for config.toml
    let resolved = &config_conflicts[0];
    match &resolved.resolution {
        Resolution::RenameIncoming { new_path } => {
            let name = new_path.to_string_lossy();
            assert!(
                name.starts_with("config.toml@"),
                "Conflict file should be config.toml@<hash>, got: {}",
                name
            );
        }
        other => panic!(
            "Expected RenameIncoming for config.toml conflict, got {:?}",
            other
        ),
    }

    // Verify Alice still has her files
    assert!(alice.cat(&PathBuf::from("/README.md")).await.is_ok());
    assert!(alice.cat(&PathBuf::from("/src/main.rs")).await.is_ok());
    assert!(alice.cat(&PathBuf::from("/src/utils.rs")).await.is_ok());
    assert!(alice.cat(&PathBuf::from("/config.toml")).await.is_ok());
}

/// Test the original low-level merge_logs API still works correctly
/// This ensures backward compatibility for code using the direct ops_log merge
#[tokio::test]
async fn test_multi_version_divergence_low_level() {
    let (mut alice, blobs, _, _temp) = common::setup_test_env().await;
    let (mut bob, bob_key) = common::fork_mount(&mut alice, &blobs).await;

    // Alice's chain: 3 versions of changes
    alice
        .add(
            &PathBuf::from("/README.md"),
            Cursor::new(b"Alice README".to_vec()),
        )
        .await
        .unwrap();
    alice.mkdir(&PathBuf::from("/src")).await.unwrap();
    alice.save(&blobs, None).await.unwrap();

    alice
        .add(
            &PathBuf::from("/src/main.rs"),
            Cursor::new(b"fn main() {}".to_vec()),
        )
        .await
        .unwrap();
    alice
        .add(
            &PathBuf::from("/src/utils.rs"),
            Cursor::new(b"pub fn help() {}".to_vec()),
        )
        .await
        .unwrap();
    alice.save(&blobs, None).await.unwrap();

    alice
        .add(
            &PathBuf::from("/config.toml"),
            Cursor::new(b"[alice]".to_vec()),
        )
        .await
        .unwrap();
    alice.save(&blobs, None).await.unwrap();

    // Bob's chain: 3 versions, diverging from the same v0
    bob.add(
        &PathBuf::from("/CONTRIBUTING.md"),
        Cursor::new(b"Bob contrib".to_vec()),
    )
    .await
    .unwrap();
    bob.mkdir(&PathBuf::from("/src")).await.unwrap();
    bob.save(&blobs, None).await.unwrap();

    bob.add(
        &PathBuf::from("/src/lib.rs"),
        Cursor::new(b"pub mod tests;".to_vec()),
    )
    .await
    .unwrap();
    bob.save(&blobs, None).await.unwrap();

    bob.add(
        &PathBuf::from("/config.toml"),
        Cursor::new(b"[bob]".to_vec()),
    )
    .await
    .unwrap();
    bob.add(
        &PathBuf::from("/.gitignore"),
        Cursor::new(b"target/".to_vec()),
    )
    .await
    .unwrap();
    bob.save(&blobs, None).await.unwrap();

    // Collect ops using the new collect_ops_since API
    let alice_ops = alice.collect_ops_since(None, &blobs).await.unwrap();
    let bob_ops = bob.collect_ops_since(None, &blobs).await.unwrap();

    // Sanity check: both peers should have accumulated ops across their saves
    assert!(
        alice_ops.len() >= 5,
        "Alice should have at least 5 ops, got {}",
        alice_ops.len()
    );
    assert!(
        bob_ops.len() >= 5,
        "Bob should have at least 5 ops, got {}",
        bob_ops.len()
    );

    let resolver = ConflictFile::new();
    let (merged, results) = merge_logs(&[&alice_ops, &bob_ops], &resolver, &bob_key.public());

    // Only config.toml conflicts
    let config_conflicts: Vec<_> = results[0]
        .conflicts_resolved
        .iter()
        .filter(|c| c.conflict.path.to_string_lossy().contains("config"))
        .collect();
    assert_eq!(config_conflicts.len(), 1);

    // All files exist
    let final_state = merged.resolve_all();
    assert!(final_state.contains_key(&PathBuf::from("README.md")));
    assert!(final_state.contains_key(&PathBuf::from("CONTRIBUTING.md")));
    assert!(final_state.contains_key(&PathBuf::from("src/main.rs")));
    assert!(final_state.contains_key(&PathBuf::from("src/lib.rs")));
    assert!(final_state.contains_key(&PathBuf::from("config.toml")));
    assert!(final_state.contains_key(&PathBuf::from(".gitignore")));
}
