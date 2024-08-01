/*
 * Copyright (c) Radical HQ Limited
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::{
    collections::{HashMap, HashSet, VecDeque},
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::{
    config::Config,
    error::{Error, Result, ResultExt},
    github::GitHubBranch,
    message::{
        build_commit_message, parse_message, MessageSection, MessageSectionsMap,
    },
    utils::run_command,
};
use debug_ignore::DebugIgnore;
use git2::Oid;
use git2_ext::ops::UserSign;

#[derive(Debug)]
pub struct PreparedCommit {
    pub oid: Oid,
    pub short_id: String,
    pub parent_oid: Oid,
    pub message: MessageSectionsMap,
    pub pull_request_number: Option<u64>,
}

#[derive(Clone)]
pub struct Git {
    repo: std::sync::Arc<std::sync::Mutex<GitRepo>>,
    jj: Option<JujutsuRepo>,
}

impl Git {
    pub fn new(repo: git2::Repository) -> Result<Self> {
        // XXX: should print debug logging if a jj repo isn't found.
        let jj = match JujutsuRepo::from_git_path(repo.path()) {
            Ok(cli) => {
                eprintln!("info: using jujutsu backend");
                Some(cli)
            }
            Err(error) => {
                #[cfg(debug_assertions)]
                {
                    let mut messages = error.messages().iter();
                    let mut combined =
                        messages.next().cloned().unwrap_or_default();
                    for message in messages {
                        combined.push_str("\n Caused by: ");
                        combined.push_str(message);
                    }

                    eprintln!("info: not using jj, because {}\n", combined);
                }
                None
            }
        };
        Ok(Self {
            repo: std::sync::Arc::new(std::sync::Mutex::new(GitRepo::new(
                repo,
            )?)),
            jj,
        })
    }

    pub(crate) fn repo(&self) -> std::sync::MutexGuard<GitRepo> {
        self.repo.lock().expect("poisoned mutex")
    }

    pub fn get_commit_oids(&self, master_ref: &str) -> Result<Vec<Oid>> {
        let repo = self.repo();
        let mut walk = repo.revwalk()?;
        walk.set_sorting(git2::Sort::TOPOLOGICAL.union(git2::Sort::REVERSE))?;
        walk.push_head()?;
        walk.hide_ref(master_ref)?;

        Ok(walk.collect::<std::result::Result<Vec<Oid>, _>>()?)
    }

    pub fn get_prepared_commits(
        &self,
        config: &Config,
    ) -> Result<Vec<PreparedCommit>> {
        self.get_commit_oids(config.master_ref.local())?
            .into_iter()
            .map(|oid| self.prepare_commit(config, oid))
            .collect()
    }

    pub fn rewrite_commit_messages(
        &self,
        commits: &mut [PreparedCommit],
        mut limit: Option<usize>,
    ) -> Result<()> {
        if commits.is_empty() {
            return Ok(());
        }

        if let Some(jj) = &self.jj {
            // XXX we don't yet support the limit parameter, since that's not currently used by any
            // of the callers
            return jj.rewrite_commit_messages(commits);
        }

        let mut parent_oid: Option<Oid> = None;
        let mut updating = false;
        let mut message: String;
        let first_parent = commits[0].parent_oid;
        let repo = self.repo();

        for prepared_commit in commits.iter_mut() {
            let commit = repo.find_commit(prepared_commit.oid)?;
            if limit != Some(0) {
                message = build_commit_message(&prepared_commit.message);
                if Some(&message[..]) != commit.message() {
                    updating = true;
                }
            } else {
                if !updating {
                    return Ok(());
                }
                message = String::from_utf8_lossy(commit.message_bytes())
                    .into_owned();
            }
            limit = limit.map(|n| if n > 0 { n - 1 } else { 0 });

            if updating {
                let new_oid = repo.commit(
                    &commit.author(),
                    &commit.committer(),
                    &message[..],
                    &commit.tree()?,
                    &[&repo.find_commit(parent_oid.unwrap_or(first_parent))?],
                    RunPostRewriteRebaseHooks::Yes {
                        prepared_commit: prepared_commit.oid,
                    },
                )?;
                prepared_commit.oid = new_oid;
                parent_oid = Some(new_oid);
            } else {
                parent_oid = Some(prepared_commit.oid);
            }
        }

        if updating {
            if let Some(oid) = parent_oid {
                repo.find_reference("HEAD")?
                    .resolve()?
                    .set_target(oid, "spr updated commit messages")?;
            }
        }

        Ok(())
    }

    pub fn rebase_commits(
        &self,
        commits: &mut [PreparedCommit],
        mut new_parent_oid: git2::Oid,
    ) -> Result<()> {
        if commits.is_empty() {
            return Ok(());
        }
        let repo = self.repo();

        for prepared_commit in commits.iter_mut() {
            let new_parent_commit = repo.find_commit(new_parent_oid)?;
            let commit = repo.find_commit(prepared_commit.oid)?;

            let index = repo.cherrypick_commit(&commit, &new_parent_commit)?;
            if index.has_conflicts() {
                return Err(Error::new("Rebase failed due to merge conflicts"));
            }

            let tree_oid = self.write_index(index)?;
            if tree_oid == new_parent_commit.tree_id() {
                // Rebasing makes this an empty commit. This is probably because
                // we just landed this commit. So we should run a hook as this
                // commit (the local pre-land commit) having been rewritten into
                // the parent (the freshly landed and pulled commit). Although
                // this behaviour is tuned around a land operation, it's in
                // general not an unreasoanble thing for a rebase, ala git
                // rebase --interactive and fixups etc.
                repo.run_post_rewrite_rebase_hooks(&[(
                    prepared_commit.oid,
                    new_parent_oid,
                )]);
                continue;
            }
            let tree = repo.find_tree(tree_oid)?;

            new_parent_oid = repo.commit(
                &commit.author(),
                &commit.committer(),
                String::from_utf8_lossy(commit.message_bytes()).as_ref(),
                &tree,
                &[&new_parent_commit],
                RunPostRewriteRebaseHooks::Yes {
                    prepared_commit: prepared_commit.oid,
                },
            )?;
        }

        let new_oid = new_parent_oid;
        let new_commit = repo.find_commit(new_oid)?;

        // Get and resolve the HEAD reference. This will be either a reference
        // to a branch ('refs/heads/...') or 'HEAD' if the head is detached.
        let mut reference = repo.head()?.resolve()?;

        // Checkout the tree of the top commit of the rebased branch. This can
        // fail if there are local changes in the worktree that collide with
        // files that need updating in order to check out the rebased commit. In
        // this case we fail early here, before we update any references. The
        // result is that the worktree is unchanged and neither the branch nor
        // HEAD gets updated. We can just prompt the user to rebase manually.
        // That's a fine solution. If the user tries "git rebase origin/master"
        // straight away, they will find that it also fails because of local
        // worktree changes. Once the user has dealt with those (revert, stash
        // or commit), the rebase should work nicely.
        repo.checkout_tree(new_commit.as_object())
            .map_err(Error::from)
            .reword(
                "Could not check out rebased branch - please rebase manually"
                    .into(),
            )?;

        // Update the reference. The reference may be a branch or "HEAD", if
        // detached. Either way, whatever we are on gets update to point to the
        // new commit.
        reference.set_target(new_oid, "spr rebased")?;

        Ok(())
    }

    pub fn head(&self) -> Result<Oid> {
        let oid = self
            .repo()
            .head()?
            .resolve()?
            .target()
            .ok_or_else(|| Error::new("Cannot resolve HEAD"))?;

        Ok(oid)
    }

    pub fn resolve_reference(&self, reference: &str) -> Result<Oid> {
        let result = self
            .repo()
            .find_reference(reference)?
            .peel_to_commit()?
            .id();

        Ok(result)
    }

    pub async fn fetch_commits_from_remote(
        &self,
        commit_oids: &[git2::Oid],
        remote: &str,
    ) -> Result<()> {
        let missing_commit_oids: Vec<_> = {
            let repo = self.repo();

            commit_oids
                .iter()
                .filter(|oid| repo.find_commit(**oid).is_err())
                .collect()
        };

        if !missing_commit_oids.is_empty() {
            let mut command = tokio::process::Command::new("git");
            command
                .arg("fetch")
                .arg("--no-write-fetch-head")
                .arg("--")
                .arg(remote);

            for oid in missing_commit_oids {
                command.arg(format!("{}", oid));
            }

            run_command(&mut command)
                .await
                .reword("git fetch failed".to_string())?;
        }

        Ok(())
    }

    pub async fn fetch_from_remote(
        refs: &[&GitHubBranch],
        remote: &str,
    ) -> Result<()> {
        if !refs.is_empty() {
            let mut command = tokio::process::Command::new("git");
            command
                .arg("fetch")
                .arg("--no-write-fetch-head")
                .arg("--")
                .arg(remote);

            for ghref in refs {
                command.arg(ghref.on_github());
            }

            run_command(&mut command)
                .await
                .reword("git fetch failed".to_string())?;
        }

        Ok(())
    }

    pub fn prepare_commit(
        &self,
        config: &Config,
        oid: Oid,
    ) -> Result<PreparedCommit> {
        let repo = self.repo();
        let commit = repo.find_commit(oid)?;

        if commit.parent_count() != 1 {
            return Err(Error::new("Parent commit count != 1"));
        }

        let parent_oid = commit.parent_id(0)?;

        let message =
            String::from_utf8_lossy(commit.message_bytes()).into_owned();

        let short_id =
            commit.as_object().short_id()?.as_str().unwrap().to_string();
        drop(commit);
        drop(repo);

        let mut message = parse_message(&message, MessageSection::Title);

        let pull_request_number = message
            .get(&MessageSection::PullRequest)
            .and_then(|text| config.parse_pull_request_field(text));

        if let Some(number) = pull_request_number {
            message.insert(
                MessageSection::PullRequest,
                config.pull_request_url(number),
            );
        } else {
            message.remove(&MessageSection::PullRequest);
        }

        Ok(PreparedCommit {
            oid,
            short_id,
            parent_oid,
            message,
            pull_request_number,
        })
    }

    pub fn get_all_ref_names(&self) -> Result<HashSet<String>> {
        let result: std::result::Result<HashSet<_>, _> = self
            .repo()
            .references()?
            .names()
            .map(|r| r.map(String::from))
            .collect();

        Ok(result?)
    }

    pub fn get_pr_patch_branch_name(&self, pr_number: u64) -> Result<String> {
        let ref_names = self.get_all_ref_names()?;
        let default_name = format!("PR-{}", pr_number);
        if !ref_names.contains(&format!("refs/heads/{}", default_name)) {
            return Ok(default_name);
        }

        let mut count = 1;
        loop {
            let name = format!("PR-{}-{}", pr_number, count);
            if !ref_names.contains(&format!("refs/heads/{}", name)) {
                return Ok(name);
            }
            count += 1;
        }
    }

    pub fn cherrypick(&self, oid: Oid, base_oid: Oid) -> Result<git2::Index> {
        let repo = self.repo();
        let commit = repo.find_commit(oid)?;
        let base_commit = repo.find_commit(base_oid)?;

        repo.cherrypick_commit(&commit, &base_commit)
    }

    pub fn write_index(&self, index: git2::Index) -> Result<Oid> {
        self.repo().write_index(index)
    }

    pub fn get_tree_oid_for_commit(&self, oid: Oid) -> Result<Oid> {
        let tree_oid = self.repo().find_commit(oid)?.tree_id();

        Ok(tree_oid)
    }

    pub fn find_master_base(
        &self,
        commit_oid: Oid,
        master_oid: Oid,
    ) -> Result<Option<Oid>> {
        let mut commit_ancestors = HashSet::new();
        let mut commit_oid = Some(commit_oid);
        let mut master_ancestors = HashSet::new();
        let mut master_queue = VecDeque::new();
        master_ancestors.insert(master_oid);
        master_queue.push_back(master_oid);
        let repo = self.repo();

        while !(commit_oid.is_none() && master_queue.is_empty()) {
            if let Some(oid) = commit_oid {
                if master_ancestors.contains(&oid) {
                    return Ok(Some(oid));
                }
                commit_ancestors.insert(oid);
                let commit = repo.find_commit(oid)?;
                commit_oid = match commit.parent_count() {
                    0 => None,
                    l => Some(commit.parent_id(l - 1)?),
                };
            }

            if let Some(oid) = master_queue.pop_front() {
                if commit_ancestors.contains(&oid) {
                    return Ok(Some(oid));
                }
                let commit = repo.find_commit(oid)?;
                for oid in commit.parent_ids() {
                    if !master_ancestors.contains(&oid) {
                        master_queue.push_back(oid);
                        master_ancestors.insert(oid);
                    }
                }
            }
        }

        Ok(None)
    }

    pub fn create_derived_commit(
        &self,
        original_commit_oid: Oid,
        message: &str,
        tree_oid: Oid,
        parent_oids: &[Oid],
    ) -> Result<Oid> {
        let repo = self.repo();
        let original_commit = repo.find_commit(original_commit_oid)?;
        let tree = repo.find_tree(tree_oid)?;
        let parents = parent_oids
            .iter()
            .map(|oid| repo.find_commit(*oid))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let parent_refs = parents.iter().collect::<Vec<_>>();
        let message = git2::message_prettify(message, None)?;

        // The committer signature should be the default signature (i.e. the
        // current user - as configured in Git as `user.name` and `user.email` -
        // and the timestamp set to now). If the default signature can't be
        // obtained (no user configured), then take the user/email from the
        // existing commit but make a new signature which has a timestamp of
        // now.
        let committer = repo.signature().or_else(|_| {
            git2::Signature::now(
                String::from_utf8_lossy(
                    original_commit.committer().name_bytes(),
                )
                .as_ref(),
                String::from_utf8_lossy(
                    original_commit.committer().email_bytes(),
                )
                .as_ref(),
            )
        })?;

        // The author signature should reference the same user as the original
        // commit, but we set the timestamp to now, so this commit shows up in
        // GitHub's timeline in the right place.
        let author = git2::Signature::now(
            String::from_utf8_lossy(original_commit.author().name_bytes())
                .as_ref(),
            String::from_utf8_lossy(original_commit.author().email_bytes())
                .as_ref(),
        )?;

        let oid = repo.commit(
            &author,
            &committer,
            &message,
            &tree,
            &parent_refs[..],
            RunPostRewriteRebaseHooks::No,
        )?;

        Ok(oid)
    }

    pub fn check_no_uncommitted_changes(&self) -> Result<()> {
        let mut opts = git2::StatusOptions::new();
        opts.include_ignored(false).include_untracked(false);
        if self.repo().statuses(Some(&mut opts))?.is_empty() {
            Ok(())
        } else {
            Err(Error::new(
                "There are uncommitted changes. Stash or amend them first",
            ))
        }
    }
}

#[derive(Debug)]
pub(crate) struct GitRepo {
    repo: DebugIgnore<git2::Repository>,
    hooks: git2_ext::hooks::Hooks,
    sign: CommitSign,
}

impl GitRepo {
    fn new(repo: git2::Repository) -> Result<Self> {
        let hooks = git2_ext::hooks::Hooks::with_repo(&repo)?;
        let config = repo
            .config()
            .context("failed to read repo config".to_owned())?;
        // If commit.gpgsign is set, then attempt to obtain the signing info.
        let sign = CommitSign::new(&repo, &config);

        Ok(Self {
            repo: DebugIgnore(repo),
            hooks,
            sign,
        })
    }

    fn head(&self) -> Result<git2::Reference> {
        Ok(self.repo.head()?)
    }

    pub(crate) fn set_head(&self, reference: &str) -> Result<()> {
        Ok(self.repo.set_head(reference)?)
    }

    fn signature(&self) -> Result<git2::Signature<'_>> {
        Ok(self.repo.signature()?)
    }

    fn revwalk(&self) -> Result<git2::Revwalk> {
        Ok(self.repo.revwalk()?)
    }

    pub(crate) fn find_commit(&self, oid: Oid) -> Result<git2::Commit> {
        Ok(self.repo.find_commit(oid)?)
    }

    fn find_tree(&self, oid: Oid) -> Result<git2::Tree> {
        Ok(self.repo.find_tree(oid)?)
    }

    pub(crate) fn merge_base(&self, a: Oid, b: Oid) -> Result<Oid> {
        Ok(self.repo.merge_base(a, b)?)
    }

    fn references(&self) -> Result<git2::References> {
        Ok(self.repo.references()?)
    }

    fn find_reference(&self, name: &str) -> Result<git2::Reference> {
        Ok(self.repo.find_reference(name)?)
    }

    pub(crate) fn checkout_tree(&self, treeish: &git2::Object) -> Result<()> {
        Ok(self.repo.checkout_tree(treeish, None)?)
    }

    fn commit(
        &self,
        author: &git2::Signature<'_>,
        committer: &git2::Signature<'_>,
        message: &str,
        tree: &git2::Tree<'_>,
        parents: &[&git2::Commit<'_>],
        run_post_rewrite_hooks: RunPostRewriteRebaseHooks,
    ) -> Result<Oid> {
        let sign = self.sign.as_dyn_sign();
        let new_oid = git2_ext::ops::commit(
            &self.repo, author, committer, message, tree, parents, sign,
        )?;

        match run_post_rewrite_hooks {
            RunPostRewriteRebaseHooks::Yes { prepared_commit } => {
                self.hooks.run_post_rewrite_rebase(
                    &self.repo,
                    &[(prepared_commit, new_oid)],
                );
            }
            RunPostRewriteRebaseHooks::No => {}
        };

        Ok(new_oid)
    }

    fn run_post_rewrite_rebase_hooks(&self, changed: &[(Oid, Oid)]) {
        self.hooks.run_post_rewrite_rebase(&self.repo, changed);
    }

    pub(crate) fn merge_commits(
        &self,
        our_commit: &git2::Commit<'_>,
        their_commit: &git2::Commit<'_>,
    ) -> Result<git2::Index> {
        Ok(self.repo.merge_commits(our_commit, their_commit, None)?)
    }

    pub(crate) fn force_branch(
        &self,
        name: &str,
        target: &git2::Commit<'_>,
    ) -> Result<git2::Branch> {
        Ok(self.repo.branch(name, target, true)?)
    }

    fn cherrypick_commit(
        &self,
        commit: &git2::Commit<'_>,
        base: &git2::Commit<'_>,
    ) -> Result<git2::Index> {
        Ok(self.repo.cherrypick_commit(commit, base, 0, None)?)
    }

    fn statuses(
        &self,
        opts: Option<&mut git2::StatusOptions>,
    ) -> Result<git2::Statuses<'_>> {
        Ok(self.repo.statuses(opts)?)
    }

    fn write_index(&self, mut index: git2::Index) -> Result<Oid> {
        Ok(index.write_tree_to(&self.repo)?)
    }
}

#[derive(Debug)]
enum CommitSign {
    Enabled(DebugIgnore<UserSign>),
    EnabledButError,
    Disabled,
}

impl CommitSign {
    fn new(repo: &git2::Repository, config: &git2::Config) -> Self {
        match config.get_bool("commit.gpgsign") {
            Ok(true) => match UserSign::from_config(repo, config) {
                Ok(sign) => Self::Enabled(DebugIgnore(sign)),
                Err(err) => {
                    eprintln!("[spr] unable to obtain signing info: {}", err);
                    Self::EnabledButError
                }
            },
            Ok(false) => Self::Disabled,
            Err(err) => {
                if err.code() == git2::ErrorCode::NotFound {
                    Self::Disabled
                } else {
                    eprintln!("[spr] unable to read commit.gpgsign: {}", err);
                    Self::Disabled
                }
            }
        }
    }

    fn as_dyn_sign(&self) -> Option<&dyn git2_ext::ops::Sign> {
        match self {
            Self::Enabled(sign) => Some(&**sign),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum RunPostRewriteRebaseHooks {
    Yes { prepared_commit: Oid },
    No,
}

#[derive(Clone, Debug)]
struct JujutsuRepo {
    cli: JujutsuCli,
}

impl JujutsuRepo {
    fn from_git_path(dot_git_path: &Path) -> Result<Self> {
        // This is a (colocated) jujutsu repo if:
        // - git_path ends with .git
        // - the path's parent is the same as what's returned by `jj root`

        let dot_git_path = dot_git_path.canonicalize()?;
        if !dot_git_path.ends_with(".git") {
            return Err(Error::new(format!(
                "git path {} does not end with .git",
                dot_git_path.display()
            )));
        }
        let repo_path = dot_git_path.parent().ok_or_else(|| {
            Error::new(format!(
                "git path {} has no parent",
                dot_git_path.display()
            ))
        })?;

        // This is a _potential_ jj CLI -- we need to check if the actual root lines up.
        let jj_bin = get_jj_bin();
        let cli = JujutsuCli {
            jj_bin,
            repo_path: repo_path.to_owned(),
        };

        // Try fetching the root from the CLI.
        let root = cli.run_captured_with_args(["root"])?;
        let root = Path::new(root.trim_end()).canonicalize()?;

        // Ensure that the root is the same.
        if root != repo_path {
            return Err(Error::new(format!(
                "git path {} is not colocated with jj root {}",
                dot_git_path.display(),
                root.display()
            )));
        }

        Ok(Self { cli })
    }

    fn rewrite_commit_messages(
        &self,
        commits: &[PreparedCommit],
    ) -> Result<()> {
        // Turn all the commit IDs into change IDs.
        let jj_change_data = self
            .cli
            .convert_commits_to_jj(commits.iter().map(|c| c.oid))?;

        // Use a bunch of `jj describe` operations to write out the new commit messages for each
        // change ID.
        for prepared_commit in commits {
            let change_data =
                jj_change_data.get(&prepared_commit.oid).ok_or_else(|| {
                    Error::new(format!(
                        "commit {} did not have a corresponding change ID",
                        prepared_commit.oid
                    ))
                })?;

            let new_message = build_commit_message(&prepared_commit.message);
            if new_message != change_data.description {
                let args = &[
                    "describe",
                    &change_data.change_id,
                    "--message",
                    &new_message,
                ];
                self.cli.run_captured_with_args(args)?;
            }
        }

        Ok(())
    }
}

/// CLI interface to jujutsu.
#[derive(Clone, Debug)]
struct JujutsuCli {
    jj_bin: PathBuf,
    repo_path: PathBuf,
}

impl JujutsuCli {
    fn convert_commits_to_jj<I>(
        &self,
        commit_ids: I,
    ) -> Result<HashMap<Oid, JujutsuChangeData>>
    where
        I: IntoIterator<Item = Oid>,
    {
        // Build a map from commit IDs as strings to their Oids.
        let commit_ids: HashMap<String, Oid> = commit_ids
            .into_iter()
            .map(|oid| (oid.to_string(), oid))
            .collect();

        let mut args = vec![
            "log".to_string(),
            "--no-graph".to_string(),
            "--template".to_string(),
            // We must escape especially the \0 since you can't spawn a command with a literal null
            // byte.
            "commit_id ++ \"\\t\" ++ change_id ++ \"\\n\" ++ description ++ \"\\0\"".to_string(),
        ];

        // For each revision, provide -r <rev> to jj.
        args.extend(
            commit_ids
                .keys()
                .flat_map(|commit_id| ["-r".to_string(), commit_id.clone()]),
        );

        let output = self.run_captured_with_args(&args)?;

        let mut out_map = HashMap::new();

        // The template will produce output of the form "commit_id\tchange_id\ndescription\0" for
        // each commit.

        for chunk in output.split('\0') {
            if chunk.is_empty() {
                // Likely the last line of the output.
                continue;
            }

            let (first_line, description) =
                chunk.split_once('\n').ok_or_else(|| {
                    Error::new(format!(
                    "jujutsu log output chunk did not contain a newline: {}",
                    chunk
                ))
                })?;

            let (commit_id, change_id) =
                first_line.split_once('\t').ok_or_else(|| {
                    Error::new(format!(
                        "jujutsu log output chunk did not contain a tab: {}",
                        chunk
                    ))
                })?;

            let commit_oid = commit_ids.get(commit_id).ok_or_else(|| {
                Error::new(format!(
                    "jujutsu log output contained an unknown commit ID: {}",
                    commit_id
                ))
            })?;

            out_map.insert(
                *commit_oid,
                JujutsuChangeData {
                    change_id: change_id.to_string(),
                    description: description.to_string(),
                },
            );
        }

        // Check that all the commit IDs were returned.
        if out_map.len() != commit_ids.len() {
            let missing_commit_ids: Vec<_> = commit_ids
                .iter()
                .filter(|(_, commit_oid)| !out_map.contains_key(*commit_oid))
                .map(|(commit_id, _)| commit_id.to_string())
                .collect();
            return Err(Error::new(format!(
                "jujutsu log output did not contain change IDs for all commit IDs: {}",
                missing_commit_ids.join(", ")
            )));
        }

        Ok(out_map)
    }

    fn run_captured_with_args<I, S>(&self, args: I) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new(&self.jj_bin);
        command.args(args);
        command.current_dir(&self.repo_path); // XXX: use `-R` instead?

        // Capture stdout, but let stderr go to the terminal.
        command.stdout(Stdio::piped());

        let child =
            command.spawn().context("jj failed to spawn".to_string())?;
        let output = child
            .wait_with_output()
            .context("failed to wait for jj to exit".to_string())?;
        if output.status.success() {
            let output = String::from_utf8(output.stdout)
                .context("jujutsu output was not valid UTF-8".to_string())?;
            Ok(output)
        } else {
            Err(Error::new(format!(
                "jujutsu exited with code {}, stdout:\n{}",
                output
                    .status
                    .code()
                    .map_or_else(|| "(unknown)".to_string(), |c| c.to_string()),
                String::from_utf8_lossy(&output.stdout)
            )))
        }
    }
}

struct JujutsuChangeData {
    change_id: String,
    description: String,
}

fn get_jj_bin() -> PathBuf {
    std::env::var_os("JJ").map_or_else(|| "jj".into(), |v| v.into())
}
