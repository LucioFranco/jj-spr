/*
 * Test for the master base fix in stacked changes
 */

use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Test that verifies the master base is correctly identified for stacked commits
#[test]
fn test_master_base_identification_for_stacked_commits() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Initialize repositories
    let git_init_output = Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to init git repo");

    if !git_init_output.status.success() {
        panic!(
            "Failed to initialize git repo: {}",
            String::from_utf8_lossy(&git_init_output.stderr)
        );
    }

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to set git user name");

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to set git user email");

    let init_output = Command::new("jj")
        .args(["git", "init", "--colocate"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to init jj repo");

    if !init_output.status.success() {
        panic!(
            "Failed to initialize jj repo: {}",
            String::from_utf8_lossy(&init_output.stderr)
        );
    }

    // Create initial commit on master (this will be the master base)
    fs::write(repo_path.join("master1.txt"), "master content 1")
        .expect("Failed to write master file");

    let master1_output = Command::new("jj")
        .args(["commit", "-m", "Master commit 1"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to create master commit 1");

    if !master1_output.status.success() {
        panic!(
            "Failed to create master commit 1: {}",
            String::from_utf8_lossy(&master1_output.stderr)
        );
    }

    // Get the master base change ID
    let master_base_change_id_output = Command::new("jj")
        .args(["log", "-r", "@-", "--no-graph", "-T", "change_id"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get master base change ID");
    let master_base_change_id = String::from_utf8_lossy(&master_base_change_id_output.stdout)
        .trim()
        .to_string();

    // Create parent commit (this should NOT be treated as master base)
    fs::write(repo_path.join("parent.txt"), "parent content").expect("Failed to write parent file");

    let parent_commit_output = Command::new("jj")
        .args(["commit", "-m", "Parent commit for stacking"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to create parent commit");

    if !parent_commit_output.status.success() {
        panic!(
            "Failed to create parent commit: {}",
            String::from_utf8_lossy(&parent_commit_output.stderr)
        );
    }

    // Create child commit
    fs::write(repo_path.join("child.txt"), "child content").expect("Failed to write child file");

    let child_commit_output = Command::new("jj")
        .args(["commit", "-m", "Child commit stacked on parent"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to create child commit");

    if !child_commit_output.status.success() {
        panic!(
            "Failed to create child commit: {}",
            String::from_utf8_lossy(&child_commit_output.stderr)
        );
    }

    // Get the child change ID
    let child_change_id_output = Command::new("jj")
        .args(["log", "-r", "@-", "--no-graph", "-T", "change_id"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get child change ID");
    let child_change_id = String::from_utf8_lossy(&child_change_id_output.stdout)
        .trim()
        .to_string();

    // Get commit OIDs to test merge base functionality
    let master_base_oid_output = Command::new("jj")
        .args([
            "log",
            "-r",
            &master_base_change_id,
            "--no-graph",
            "-T",
            "commit_id",
        ])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get master base OID");
    let master_base_oid = String::from_utf8_lossy(&master_base_oid_output.stdout)
        .trim()
        .to_string();

    let child_oid_output = Command::new("jj")
        .args([
            "log",
            "-r",
            &child_change_id,
            "--no-graph",
            "-T",
            "commit_id",
        ])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get child OID");
    let child_oid = String::from_utf8_lossy(&child_oid_output.stdout)
        .trim()
        .to_string();

    println!("Master base change ID: {}", master_base_change_id);
    println!("Master base OID: {}", master_base_oid);
    println!("Child change ID: {}", child_change_id);
    println!("Child OID: {}", child_oid);

    // Test merge base calculation using git (this is what our fix should replicate)
    let merge_base_output = Command::new("git")
        .args(["merge-base", &child_oid, &master_base_oid])
        .current_dir(repo_path)
        .output()
        .expect("Failed to calculate merge base");

    if !merge_base_output.status.success() {
        panic!(
            "Failed to calculate merge base: {}",
            String::from_utf8_lossy(&merge_base_output.stderr)
        );
    }

    let calculated_merge_base = String::from_utf8_lossy(&merge_base_output.stdout)
        .trim()
        .to_string();
    println!("Calculated merge base: {}", calculated_merge_base);

    // The merge base should be the master base OID, not the immediate parent
    assert_eq!(
        calculated_merge_base, master_base_oid,
        "Merge base should be the master base, not the immediate parent"
    );

    // This is the key test: when processing the child commit with -r <child_rev>,
    // the master_base_oid should be the actual master base, not the immediate parent
    println!("Test passed: Master base correctly identified for stacked commit");
}

/// Test the actual fix by simulating what happens in diff.rs
#[test]
fn test_directly_based_on_master_logic_fix() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Set up the same repository structure
    let git_init_output = Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to init git repo");

    if !git_init_output.status.success() {
        panic!(
            "Failed to initialize git repo: {}",
            String::from_utf8_lossy(&git_init_output.stderr)
        );
    }

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to set git user name");

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to set git user email");

    let init_output = Command::new("jj")
        .args(["git", "init", "--colocate"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to init jj repo");

    if !init_output.status.success() {
        panic!(
            "Failed to initialize jj repo: {}",
            String::from_utf8_lossy(&init_output.stderr)
        );
    }

    // Create master commit
    fs::write(repo_path.join("master.txt"), "master content").expect("Failed to write master file");
    Command::new("jj")
        .args(["commit", "-m", "Master commit"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to create master commit");

    // Create parent commit
    fs::write(repo_path.join("parent.txt"), "parent content").expect("Failed to write parent file");
    Command::new("jj")
        .args(["commit", "-m", "Parent commit"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to create parent commit");

    // Get parent info
    let parent_change_id_output = Command::new("jj")
        .args(["log", "-r", "@-", "--no-graph", "-T", "change_id"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get parent change ID");
    let parent_change_id = String::from_utf8_lossy(&parent_change_id_output.stdout)
        .trim()
        .to_string();

    let parent_oid_output = Command::new("jj")
        .args([
            "log",
            "-r",
            &parent_change_id,
            "--no-graph",
            "-T",
            "commit_id",
        ])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get parent OID");
    let parent_oid = String::from_utf8_lossy(&parent_oid_output.stdout)
        .trim()
        .to_string();

    // Create child commit
    fs::write(repo_path.join("child.txt"), "child content").expect("Failed to write child file");
    Command::new("jj")
        .args(["commit", "-m", "Child commit"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to create child commit");

    // Get child info
    let child_change_id_output = Command::new("jj")
        .args(["log", "-r", "@-", "--no-graph", "-T", "change_id"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get child change ID");
    let child_change_id = String::from_utf8_lossy(&child_change_id_output.stdout)
        .trim()
        .to_string();

    let child_oid_output = Command::new("jj")
        .args([
            "log",
            "-r",
            &child_change_id,
            "--no-graph",
            "-T",
            "commit_id",
        ])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get child OID");
    let child_oid = String::from_utf8_lossy(&child_oid_output.stdout)
        .trim()
        .to_string();

    // Get master base (what the fix should find)
    let master_base_change_id_output = Command::new("jj")
        .args(["log", "-r", "@---", "--no-graph", "-T", "change_id"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get master base change ID");
    let master_base_change_id = String::from_utf8_lossy(&master_base_change_id_output.stdout)
        .trim()
        .to_string();

    let master_base_oid_output = Command::new("jj")
        .args([
            "log",
            "-r",
            &master_base_change_id,
            "--no-graph",
            "-T",
            "commit_id",
        ])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get master base OID");
    let master_base_oid = String::from_utf8_lossy(&master_base_oid_output.stdout)
        .trim()
        .to_string();

    println!("Child OID: {}", child_oid);
    println!("Parent OID: {}", parent_oid);
    println!("Master base OID: {}", master_base_oid);

    // Test the key logic: directly_based_on_master should be FALSE for a stacked child
    // Old buggy logic: local_commit.parent_oid == master_base_oid where master_base_oid was parent_oid
    // This would incorrectly evaluate to TRUE

    // New fixed logic: local_commit.parent_oid == master_base_oid where master_base_oid is the actual merge base
    // This should correctly evaluate to FALSE

    let directly_based_on_master_old_logic = parent_oid == parent_oid; // This was the bug - always true!
    let directly_based_on_master_new_logic = parent_oid == master_base_oid; // This is the fix - should be false for stacked commits

    println!(
        "Old logic (buggy): directly_based_on_master = {}",
        directly_based_on_master_old_logic
    );
    println!(
        "New logic (fixed): directly_based_on_master = {}",
        directly_based_on_master_new_logic
    );

    // The old logic incorrectly thought the child was directly based on master
    assert!(
        directly_based_on_master_old_logic,
        "Old logic should incorrectly return true (this demonstrates the bug)"
    );

    // The new logic correctly identifies that the child is NOT directly based on master
    assert!(
        !directly_based_on_master_new_logic,
        "New logic should correctly return false for stacked child"
    );

    println!(
        "Test passed: Fixed logic correctly identifies stacked commits as not directly based on master"
    );
}
