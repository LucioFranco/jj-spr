/*
 * Copyright (c) Radical HQ Limited
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::{
    error::{Error, Result},
    jj::PreparedCommit,
    message::validate_commit_message,
    output::{output, write_commit_title},
};

#[derive(Debug, clap::Parser)]
pub struct AmendOptions {
    /// Amend commits in range from base to revision
    #[clap(long, short = 'a')]
    all: bool,

    /// Base revision for --all mode (if not specified, uses trunk)
    #[clap(long)]
    base: Option<String>,

    /// Jujutsu revision(s) to operate on. Can be a single revision like '@' or a range like 'main..@'
    /// If a range is provided, behaves like --all mode. If not specified, uses '@-'
    #[clap(short = 'r', long)]
    revision: Option<String>,
}

pub async fn amend(
    opts: AmendOptions,
    jj: &crate::jj::Jujutsu,
    gh: &mut crate::github::GitHub,
    config: &crate::config::Config,
) -> Result<()> {
    // Determine revision and whether to use range mode
    let (use_range_mode, base_rev, target_rev) = crate::revision_utils::parse_revision_and_range(
        opts.revision.as_deref(),
        opts.all,
        opts.base.as_deref(),
    )?;

    let mut pc = if use_range_mode {
        jj.get_prepared_commits_from_to(config, &base_rev, &target_rev)?
    } else {
        vec![jj.get_prepared_commit_for_revision(config, &target_rev)?]
    };

    if pc.is_empty() {
        output("👋", "No commits found - nothing to do. Good bye!")?;
        return Ok(());
    }

    // Request the Pull Request information for each commit (well, those that
    // declare to have Pull Requests). This list is in reverse order, so that
    // below we can pop from the vector as we iterate.
    let mut pull_requests: Vec<_> = pc
        .iter()
        .rev()
        .map(|commit: &PreparedCommit| {
            commit
                .pull_request_number
                .map(|number| tokio::spawn(gh.clone().get_pull_request(number)))
        })
        .collect();

    let mut failure = false;

    for commit in pc.iter_mut() {
        write_commit_title(commit)?;
        let pull_request = pull_requests.pop().flatten();
        if let Some(pull_request) = pull_request {
            let pull_request = pull_request.await??;
            commit.message = pull_request.sections;
            commit.message_changed = true;
        }
        failure = validate_commit_message(&commit.message, config).is_err() || failure;
    }
    jj.rewrite_commit_messages(&mut pc)?;

    if failure { Err(Error::empty()) } else { Ok(()) }
}
