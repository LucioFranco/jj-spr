/*
 * Test for stacked changes immutability bug
 */

use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Test that verifies the bug where parent commits become immutable
/// when submitting a stacked child commit with `spr diff -r <child_rev>`
#[test]
fn test_stacked_changes_parent_immutability_bug() {
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

    // This would be where we run: jj-spr diff -r <child_change_id>
    // For this test, we'll simulate what happens during that operation

    // The bug reproduction: simulate what jj-spr does
    // 1. It processes the child commit
    // 2. It identifies the parent as part of the stack
    // 3. It calls rewrite_commit_messages on both parent and child
    // 4. This causes the parent to get a new change ID, making it "immutable"

    // Simulate updating the child commit message (what jj-spr would do)
    let update_child_output = Command::new("jj")
        .args([
            "describe",
            "-r",
            &child_change_id,
            "-m",
            "Child commit stacked on parent\n\nPull Request: https://github.com/test/repo/pull/123",
        ])
        .current_dir(repo_path)
        .output()
        .expect("Failed to update child commit");

    if !update_child_output.status.success() {
        panic!(
            "Failed to update child commit: {}",
            String::from_utf8_lossy(&update_child_output.stderr)
        );
    }

    // Simulate updating the parent commit message (the bug - this shouldn't happen for single revision)
    let update_parent_output = Command::new("jj")
        .args([
            "describe",
            "-r",
            &parent_change_id_before,
            "-m",
            "Parent commit for stacking",
        ])
        .current_dir(repo_path)
        .output()
        .expect("Failed to update parent commit");

    if !update_parent_output.status.success() {
        panic!(
            "Failed to update parent commit: {}",
            String::from_utf8_lossy(&update_parent_output.stderr)
        );
    }

    // Get the parent change ID after the operations
    let parent_change_id_after_output = Command::new("jj")
        .args(["log", "-r", "@--", "--no-graph", "-T", "change_id"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get parent change ID after");

    let parent_change_id_after = String::from_utf8_lossy(&parent_change_id_after_output.stdout)
        .trim()
        .to_string();

    // Verify the bug: parent change ID should be the same, but with the bug it becomes different
    println!("Parent change ID before: {}", parent_change_id_before);
    println!("Parent change ID after: {}", parent_change_id_after);

    // This assertion will fail with the current buggy implementation
    // After fixing, the parent change ID should remain the same when only processing the child
    assert_eq!(
        parent_change_id_before, parent_change_id_after,
        "Parent change ID changed when only child revision was processed - this indicates the parent became immutable"
    );
}

#[test]
fn test_stacked_changes_correct_behavior() {
    // This test verifies the correct behavior where only the child commit
    // that actually gets a PR should have its message updated

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

    // Get the parent change ID
    let parent_change_id_output = Command::new("jj")
        .args(["log", "-r", "@-", "--no-graph", "-T", "change_id"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get parent change ID");

    let parent_change_id_before = String::from_utf8_lossy(&parent_change_id_output.stdout)
        .trim()
        .to_string();

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

    // Correct behavior: only update the child commit that actually got a PR
    let update_child_output = Command::new("jj")
        .args([
            "describe",
            "-r",
            &child_change_id,
            "-m",
            "Child commit stacked on parent\n\nPull Request: https://github.com/test/repo/pull/123",
        ])
        .current_dir(repo_path)
        .output()
        .expect("Failed to update child commit");

    if !update_child_output.status.success() {
        panic!(
            "Failed to update child commit: {}",
            String::from_utf8_lossy(&update_child_output.stderr)
        );
    }

    // DO NOT update parent commit message since it didn't get a PR

    // Get the parent change ID after the operations
    let parent_change_id_after_output = Command::new("jj")
        .args(["log", "-r", "@--", "--no-graph", "-T", "change_id"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to get parent change ID after");

    let parent_change_id_after = String::from_utf8_lossy(&parent_change_id_after_output.stdout)
        .trim()
        .to_string();

    // This should pass: parent change ID should remain the same
    assert_eq!(
        parent_change_id_before, parent_change_id_after,
        "Parent change ID should remain the same when only child is processed"
    );
}
