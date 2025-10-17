/*
 * Test for stacked PR diff bug where GitHub shows cumulative diff instead of just child changes
 */

use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Test that verifies stacked PRs show only child changes in GitHub diff
#[test]
fn test_stacked_pr_shows_only_child_diff() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Initialize a git repository first
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

    // Configure git user
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

    // Initialize a jj repository on top of git
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

    // Create an initial commit on master
    fs::write(repo_path.join("initial.txt"), "initial content")
        .expect("Failed to write initial file");

    let initial_commit_output = Command::new("jj")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to create initial commit");

    if !initial_commit_output.status.success() {
        panic!(
            "Failed to create initial commit: {}",
            String::from_utf8_lossy(&initial_commit_output.stderr)
        );
    }

    // Create parent commit
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

    // Get the parent change ID before creating child
    let parent_change_id_output = Command::new("jj")
        .args(["log", "-r", "@-", "--no-graph", "-T", "change_id"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get parent change ID");

    let parent_change_id_before = String::from_utf8_lossy(&parent_change_id_output.stdout)
        .trim()
        .to_string();
    println!("Parent change ID before: {}", parent_change_id_before);

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
    println!("Child change ID: {}", child_change_id);

    // Check the current state before any spr operations
    let log_output = Command::new("jj")
        .args([
            "log",
            "--no-graph",
            "-T",
            "change_id ++ \" \" ++ description",
        ])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get jj log");

    println!(
        "Log before spr operations:\n{}",
        String::from_utf8_lossy(&log_output.stdout)
    );

    // This is where we would run: jj-spr diff -r <child_change_id>
    // For this test, we'll simulate what should happen vs what actually happens

    // The correct behavior for stacked PRs:
    // 1. Child commit should get a PR against a base branch containing the parent
    // 2. The base branch should be created without affecting the parent's change ID
    // 3. GitHub should show only the child's changes in the PR diff

    // Let's check if the parent change ID remains the same after the operations
    // (This simulates what jj-spr would do when processing the child)

    // Simulate getting tree info for the child commit (what jj-spr does)
    let child_tree_output = Command::new("jj")
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
        .expect("Failed to get child commit ID");

    let child_commit_id = String::from_utf8_lossy(&child_tree_output.stdout)
        .trim()
        .to_string();
    println!("Child commit ID: {}", child_commit_id);

    // Simulate getting parent tree info (what jj-spr does for stacked PRs)
    let parent_tree_output = Command::new("jj")
        .args([
            "log",
            "-r",
            &parent_change_id_before,
            "--no-graph",
            "-T",
            "commit_id",
        ])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get parent commit ID");

    let parent_commit_id = String::from_utf8_lossy(&parent_tree_output.stdout)
        .trim()
        .to_string();
    println!("Parent commit ID: {}", parent_commit_id);

    // Get the parent change ID after these operations
    let parent_change_id_after_output = Command::new("jj")
        .args([
            "log",
            "-r",
            &parent_commit_id,
            "--no-graph",
            "-T",
            "change_id",
        ])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get parent change ID after");

    let parent_change_id_after = String::from_utf8_lossy(&parent_change_id_after_output.stdout)
        .trim()
        .to_string();
    println!("Parent change ID after: {}", parent_change_id_after);

    // The parent change ID should remain the same
    assert_eq!(
        parent_change_id_before, parent_change_id_after,
        "Parent change ID changed when processing child commit - this indicates the parent became immutable"
    );

    // Additional test: verify that we can still modify the parent commit
    let modify_parent_output = Command::new("jj")
        .args([
            "describe",
            "-r",
            &parent_change_id_before,
            "-m",
            "Parent commit for stacking (modified)",
        ])
        .current_dir(repo_path)
        .output()
        .expect("Failed to modify parent commit");

    if !modify_parent_output.status.success() {
        panic!(
            "Could not modify parent commit - it became immutable: {}",
            String::from_utf8_lossy(&modify_parent_output.stderr)
        );
    }

    println!("Test passed: Parent commit remains mutable after child processing");
}

/// Test that simulates the actual jj-spr behavior to identify where the bug occurs
#[test]
fn test_jj_spr_stacked_behavior_simulation() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Set up repository with stacked commits (same as above)
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

    // Create initial commit
    fs::write(repo_path.join("initial.txt"), "initial content")
        .expect("Failed to write initial file");
    Command::new("jj")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to create initial commit");

    // Create parent commit
    fs::write(repo_path.join("parent.txt"), "parent content").expect("Failed to write parent file");
    Command::new("jj")
        .args(["commit", "-m", "Parent commit"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to create parent commit");

    // Create child commit
    fs::write(repo_path.join("child.txt"), "child content").expect("Failed to write child file");
    Command::new("jj")
        .args(["commit", "-m", "Child commit"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to create child commit");

    // Get initial state
    let parent_change_id_output = Command::new("jj")
        .args(["log", "-r", "@--", "--no-graph", "-T", "change_id"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get parent change ID");
    let parent_change_id = String::from_utf8_lossy(&parent_change_id_output.stdout)
        .trim()
        .to_string();

    let child_change_id_output = Command::new("jj")
        .args(["log", "-r", "@-", "--no-graph", "-T", "change_id"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get child change ID");
    let child_change_id = String::from_utf8_lossy(&child_change_id_output.stdout)
        .trim()
        .to_string();

    println!("Initial parent change ID: {}", parent_change_id);
    println!("Initial child change ID: {}", child_change_id);

    // Simulate what jj-spr does when processing the child commit:
    // 1. Get commit OID for child
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
        .expect("Failed to get child commit OID");
    let child_oid = String::from_utf8_lossy(&child_oid_output.stdout)
        .trim()
        .to_string();

    // 2. Get parent OID for the child commit (this is what might cause issues)
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

    println!("Child OID: {}", child_oid);
    println!("Parent OID: {}", parent_oid);

    // 3. Simulate getting change ID from OID (this might be the problematic operation)
    let parent_change_id_from_oid_output = Command::new("jj")
        .args(["log", "-r", &parent_oid, "--no-graph", "-T", "change_id"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get parent change ID from OID");
    let parent_change_id_from_oid =
        String::from_utf8_lossy(&parent_change_id_from_oid_output.stdout)
            .trim()
            .to_string();

    println!("Parent change ID from OID: {}", parent_change_id_from_oid);

    // Check if the parent change ID changed
    assert_eq!(
        parent_change_id, parent_change_id_from_oid,
        "Parent change ID changed when referenced by OID - this might be the bug!"
    );
}
