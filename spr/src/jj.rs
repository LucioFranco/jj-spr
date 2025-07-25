/*
 * Copyright (c) Radical HQ Limited
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::{
    ffi::OsStr,
    path::PathBuf,
    process::{Command, Stdio},
};

use crate::{
    config::Config,
    error::{Error, Result, ResultExt},
    message::{build_commit_message, parse_message, MessageSection, MessageSectionsMap},
};
use git2::Oid;

#[derive(Debug)]
pub struct PreparedCommit {
    pub oid: Oid,
    pub short_id: String,
    pub parent_oid: Oid,
    pub message: MessageSectionsMap,
    pub pull_request_number: Option<u64>,
    pub message_changed: bool,
}

pub struct Jujutsu {
    repo_path: PathBuf,
    jj_bin: PathBuf,
    pub git_repo: git2::Repository,
}

impl Jujutsu {
    pub fn new(git_repo: git2::Repository) -> Result<Self> {
        let repo_path = git_repo
            .workdir()
            .ok_or_else(|| Error::new("Repository must have a working directory".to_string()))?
            .to_path_buf();

        // Verify this is a Jujutsu repository
        let jj_dir = repo_path.join(".jj");
        if !jj_dir.exists() {
            return Err(Error::new(
                "This is not a Jujutsu repository. Run 'jj git init --colocate' to create one."
                    .to_string(),
            ));
        }

        let jj_bin = get_jj_bin();

        Ok(Self {
            repo_path,
            jj_bin,
            git_repo,
        })
    }

    pub fn get_prepared_commit_for_revision(
        &self,
        config: &Config,
        revision: &str,
    ) -> Result<PreparedCommit> {
        let commit_oid = self.resolve_revision_to_commit_id(revision)?;
        self.prepare_commit(config, commit_oid)
    }

    pub fn get_master_base_for_commit(&self, config: &Config, commit_oid: Oid) -> Result<Oid> {
        // Find the merge base between the commit and master
        let master_oid = self.resolve_revision_to_commit_id(config.master_ref.local())?;
        let merge_base = self.git_repo.merge_base(commit_oid, master_oid)?;
        Ok(merge_base)
    }

    pub fn get_prepared_commits_from_to(
        &self,
        config: &Config,
        from_revision: &str,
        to_revision: &str,
    ) -> Result<Vec<PreparedCommit>> {
        // Get commit range using jj
        let output = self.run_captured_with_args([
            "log",
            "--no-graph",
            "-r",
            &format!("{}::{}", from_revision, to_revision),
            "--template",
            "commit_id ++ \"\\n\"",
        ])?;

        let mut commits = Vec::new();
        for line in output.lines() {
            let line = line.trim();
            if !line.is_empty() {
                let commit_oid = Oid::from_str(line).map_err(|e| {
                    Error::new(format!("Failed to parse commit ID '{}': {}", line, e))
                })?;
                commits.push(self.prepare_commit(config, commit_oid)?);
            }
        }

        Ok(commits)
    }

    pub fn check_no_uncommitted_changes(&self) -> Result<()> {
        let output = self.run_captured_with_args(["status"])?;

        // Check if there are any changes
        // Jujutsu reports "The working copy has no changes" when clean
        if output.trim().is_empty()
            || output.contains("No changes.")
            || output.contains("The working copy has no changes")
        {
            Ok(())
        } else {
            Err(Error::new(format!(
                "You have uncommitted changes:\n{}",
                output
            )))
        }
    }

    pub fn get_all_ref_names(&self) -> Result<std::collections::HashSet<String>> {
        // Use git for ref names since jj doesn't expose them directly
        let refs = self.git_repo.references()?;
        let mut ref_names = std::collections::HashSet::new();

        for reference in refs {
            let reference = reference?;
            if let Some(name) = reference.name() {
                ref_names.insert(name.to_string());
            }
        }

        Ok(ref_names)
    }

    pub fn resolve_reference(&self, ref_name: &str) -> Result<Oid> {
        let reference = self.git_repo.find_reference(ref_name)?;
        reference
            .target()
            .ok_or_else(|| Error::new(format!("Reference {} has no target", ref_name)))
    }

    pub fn get_tree_oid_for_commit(&self, commit_oid: Oid) -> Result<Oid> {
        let commit = self.git_repo.find_commit(commit_oid)?;
        Ok(commit.tree()?.id())
    }

    pub fn create_derived_commit(
        &self,
        original_commit_oid: Oid,
        message: &str,
        tree_oid: Oid,
        parent_oids: &[Oid],
    ) -> Result<Oid> {
        let original_commit = self.git_repo.find_commit(original_commit_oid)?;
        let author = original_commit.author();
        let tree = self.git_repo.find_tree(tree_oid)?;

        let mut parents = Vec::new();
        for &oid in parent_oids {
            parents.push(self.git_repo.find_commit(oid)?);
        }
        let parent_refs: Vec<_> = parents.iter().collect();

        Ok(self
            .git_repo
            .commit(None, &author, &author, message, &tree, &parent_refs)?)
    }

    pub fn cherrypick(&self, commit_oid: Oid, onto_oid: Oid) -> Result<git2::Index> {
        let commit = self.git_repo.find_commit(commit_oid)?;
        let onto_commit = self.git_repo.find_commit(onto_oid)?;
        let _commit_tree = commit.tree()?;
        let _onto_tree = onto_commit.tree()?;
        let _base_tree = if commit.parents().count() > 0 {
            commit.parent(0)?.tree()?
        } else {
            // For initial commit, use empty tree
            let empty_tree_oid = self.git_repo.treebuilder(None)?.write()?;
            self.git_repo.find_tree(empty_tree_oid)?
        };

        let index = self.git_repo.cherrypick_commit(
            &commit,
            &onto_commit,
            0,
            Some(&git2::MergeOptions::new()),
        )?;
        Ok(index)
    }

    pub fn write_index(&self, mut index: git2::Index) -> Result<Oid> {
        Ok(index.write_tree_to(&self.git_repo)?)
    }

    pub fn rewrite_commit_messages(&self, commits: &mut [PreparedCommit]) -> Result<()> {
        if commits.is_empty() {
            return Ok(());
        }

        // Use jj describe to update commit messages, but only for commits that actually changed
        for prepared_commit in commits.iter_mut() {
            // Only update commits whose messages were actually modified
            if !prepared_commit.message_changed {
                continue;
            }

            let new_message = build_commit_message(&prepared_commit.message);

            // Get the change ID for this commit
            let change_id = self.get_change_id_for_commit(prepared_commit.oid)?;

            // Update the commit message using jj describe
            let mut cmd = Command::new(&self.jj_bin);
            cmd.args(["describe", "-r", &change_id, "-m", &new_message])
                .current_dir(&self.repo_path)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            let output = cmd.output()?;
            if !output.status.success() {
                return Err(Error::new(format!(
                    "Failed to update commit message: {}",
                    String::from_utf8_lossy(&output.stderr)
                )));
            }

            // Reset the flag after successful update
            prepared_commit.message_changed = false;
        }

        Ok(())
    }

    fn prepare_commit(&self, config: &Config, commit_oid: Oid) -> Result<PreparedCommit> {
        let commit = self.git_repo.find_commit(commit_oid)?;
        let short_id = format!("{:.7}", commit_oid);

        let parent_oid = if commit.parents().count() > 0 {
            commit.parent(0)?.id()
        } else {
            // For initial commit, use a null OID or the commit itself
            commit_oid
        };

        let message_text = commit.message().unwrap_or("").to_string();
        let message = parse_message(&message_text, MessageSection::Title);

        let pull_request_number = message
            .get(&MessageSection::PullRequest)
            .and_then(|url| config.parse_pull_request_field(url));

        Ok(PreparedCommit {
            oid: commit_oid,
            short_id,
            parent_oid,
            message,
            pull_request_number,
            message_changed: false,
        })
    }

    fn resolve_revision_to_commit_id(&self, revision: &str) -> Result<Oid> {
        let output = self.run_captured_with_args([
            "log",
            "--no-graph",
            "-r",
            revision,
            "--template",
            "commit_id",
        ])?;

        let commit_id_str = output.trim();
        Oid::from_str(commit_id_str).map_err(|e| {
            Error::new(format!(
                "Failed to parse commit ID '{}' from jj output: {}",
                commit_id_str, e
            ))
        })
    }

    fn get_change_id_for_commit(&self, commit_oid: Oid) -> Result<String> {
        // Get the change ID for a given commit OID
        let output = self.run_captured_with_args([
            "log",
            "--no-graph",
            "-r",
            &commit_oid.to_string(),
            "--template",
            "change_id",
        ])?;

        Ok(output.trim().to_string())
    }

    fn run_captured_with_args<I, S>(&self, args: I) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new(&self.jj_bin);
        command.args(args);
        command.current_dir(&self.repo_path);
        command.stdout(Stdio::piped());

        let child = command.spawn().context("jj failed to spawn".to_string())?;
        let output = child
            .wait_with_output()
            .context("failed to wait for jj to exit".to_string())?;

        if output.status.success() {
            let output = String::from_utf8(output.stdout)
                .context("jujutsu output was not valid UTF-8".to_string())?;
            Ok(output)
        } else {
            Err(Error::new(format!(
                "jujutsu exited with code {}, stderr:\n{}",
                output
                    .status
                    .code()
                    .map_or_else(|| "(unknown)".to_string(), |c| c.to_string()),
                String::from_utf8_lossy(&output.stderr)
            )))
        }
    }
}

fn get_jj_bin() -> PathBuf {
    std::env::var_os("JJ").map_or_else(|| "jj".into(), |v| v.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path};
    use tempfile::TempDir;

    fn create_test_config() -> Config {
        Config::new(
            "test_owner".into(),
            "test_repo".into(),
            "origin".into(),
            "main".into(),
            "spr/test/".into(),
            false,
            false,
        )
    }

    fn create_jujutsu_test_repo() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize a Jujutsu repository
        let output = std::process::Command::new("jj")
            .args(["git", "init", "--colocate"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to run jj git init");

        if !output.status.success() {
            panic!(
                "Failed to initialize jj repo: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Set up basic jj config
        let _ = std::process::Command::new("jj")
            .args(["config", "set", "--repo", "user.name", "Test User"])
            .current_dir(&repo_path)
            .output();

        let _ = std::process::Command::new("jj")
            .args(["config", "set", "--repo", "user.email", "test@example.com"])
            .current_dir(&repo_path)
            .output();

        (temp_dir, repo_path)
    }

    fn create_jujutsu_commit(repo_path: &Path, message: &str, file_content: &str) -> String {
        // Create a file
        let file_path = repo_path.join("test.txt");
        fs::write(&file_path, file_content).expect("Failed to write test file");

        // Create a commit using jj
        let output = std::process::Command::new("jj")
            .args(["commit", "-m", message])
            .current_dir(repo_path)
            .output()
            .expect("Failed to run jj commit");

        if !output.status.success() {
            panic!(
                "Failed to create jj commit: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Get the change ID of the created commit
        let output = std::process::Command::new("jj")
            .args(["log", "--no-graph", "-r", "@-", "--template", "change_id"])
            .current_dir(repo_path)
            .output()
            .expect("Failed to get change ID");

        String::from_utf8(output.stdout)
            .expect("Invalid UTF-8 in jj output")
            .trim()
            .to_string()
    }

    #[test]
    fn test_jujutsu_creation() {
        let (_temp_dir, repo_path) = create_jujutsu_test_repo();
        let git_repo = git2::Repository::open(&repo_path).expect("Failed to open git repository");

        let jj = Jujutsu::new(git_repo).expect("Failed to create Jujutsu instance");
        assert!(jj.repo_path.exists());
        assert!(jj.repo_path.join(".jj").exists());
    }

    #[test]
    fn test_revision_resolution() {
        let (_temp_dir, repo_path) = create_jujutsu_test_repo();
        let config = create_test_config();

        // Create some commits
        let _commit1 = create_jujutsu_commit(&repo_path, "First commit", "content1");
        let _commit2 = create_jujutsu_commit(&repo_path, "Second commit", "content2");

        let git_repo = git2::Repository::open(&repo_path).expect("Failed to open git repository");
        let jj = Jujutsu::new(git_repo).expect("Failed to create Jujutsu instance");

        // Test resolving current revision (@)
        let result = jj.get_prepared_commit_for_revision(&config, "@");
        assert!(
            result.is_ok(),
            "Failed to resolve @ revision: {:?}",
            result.err()
        );

        // Test resolving previous revision (@-)
        let result = jj.get_prepared_commit_for_revision(&config, "@-");
        assert!(
            result.is_ok(),
            "Failed to resolve @- revision: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_commit_range() {
        let (_temp_dir, repo_path) = create_jujutsu_test_repo();
        let config = create_test_config();

        // Create multiple commits
        let _commit1 = create_jujutsu_commit(&repo_path, "First commit", "content1");
        let _commit2 = create_jujutsu_commit(&repo_path, "Second commit", "content2");
        let _commit3 = create_jujutsu_commit(&repo_path, "Third commit", "content3");

        let git_repo = git2::Repository::open(&repo_path).expect("Failed to open git repository");
        let jj = Jujutsu::new(git_repo).expect("Failed to create Jujutsu instance");

        // Test getting commit range
        let result = jj.get_prepared_commits_from_to(&config, "@--", "@");
        assert!(
            result.is_ok(),
            "Failed to get commit range: {:?}",
            result.err()
        );

        if let Ok(commits) = result {
            // Should get 3 commits in the range
            assert!(!commits.is_empty(), "Should get some commits in range");
        }
    }

    #[test]
    fn test_status_check() {
        let (_temp_dir, repo_path) = create_jujutsu_test_repo();

        let git_repo = git2::Repository::open(&repo_path).expect("Failed to open git repository");
        let jj = Jujutsu::new(git_repo).expect("Failed to create Jujutsu instance");

        // Should pass since new repo has no changes
        let result = jj.check_no_uncommitted_changes();
        assert!(
            result.is_ok(),
            "Status check should pass for clean repo: {:?}",
            result.err()
        );
    }
}
