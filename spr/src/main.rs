/*
 * Copyright (c) Radical HQ Limited
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! A Jujutsu subcommand for submitting and updating GitHub Pull Requests from
//! local Jujutsu commits that may be amended and rebased. Pull Requests can be
//! stacked to allow for a series of code reviews of interdependent code.

use clap::{Parser, Subcommand};
use reqwest::{self, header};
use jj_spr::{
    commands,
    error::{Error, Result, ResultExt},
    output::output,
};

// Helper function to get config value from jj first, then git
fn get_config_value(key: &str, git_config: &git2::Config) -> Option<String> {
    // Try jj config first
    if let Ok(output) = std::process::Command::new("jj")
        .args(["config", "get", key])
        .output()
    {
        if output.status.success() {
            if let Ok(value) = String::from_utf8(output.stdout) {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    
    // Fall back to git config
    git_config.get_string(key).ok()
}

fn get_config_bool(key: &str, git_config: &git2::Config) -> Option<bool> {
    // Try jj config first
    if let Ok(output) = std::process::Command::new("jj")
        .args(["config", "get", key])
        .output()
    {
        if output.status.success() {
            if let Ok(value) = String::from_utf8(output.stdout) {
                let trimmed = value.trim().to_lowercase();
                if trimmed == "true" {
                    return Some(true);
                } else if trimmed == "false" {
                    return Some(false);
                }
            }
        }
    }
    
    // Fall back to git config
    git_config.get_bool(key).ok()
}

#[derive(Parser, Debug)]
#[clap(
    name = "jj-spr",
    version,
    about = "Jujutsu subcommand: Submit pull requests for individual, amendable, rebaseable commits to GitHub"
)]
pub struct Cli {
    /// GitHub personal access token (if not given taken from jj config
    /// spr.githubAuthToken)
    #[clap(long)]
    github_auth_token: Option<String>,

    /// GitHub repository ('org/name', if not given taken from config
    /// spr.githubRepository)
    #[clap(long)]
    github_repository: Option<String>,

    /// prefix to be used for branches created for pull requests (if not given
    /// taken from jj config spr.branchPrefix, defaulting to
    /// 'spr/<GITHUB_USERNAME>/')
    #[clap(long)]
    branch_prefix: Option<String>,

    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Interactive assistant for configuring spr in a local GitHub-backed Git
    /// repository
    Init,

    /// Create a new or update an existing Pull Request on GitHub from the
    /// current HEAD commit
    Diff(commands::diff::DiffOptions),

    /// Reformat commit message
    Format(commands::format::FormatOptions),

    /// Land a reviewed Pull Request
    Land(commands::land::LandOptions),

    /// Update local commit message with content on GitHub
    Amend(commands::amend::AmendOptions),

    /// List open Pull Requests on GitHub and their review decision
    List,

    /// Create a new branch with the contents of an existing Pull Request
    Patch(commands::patch::PatchOptions),

    /// Close a Pull request
    Close(commands::close::CloseOptions),
}

#[derive(Debug, thiserror::Error)]
pub enum OptionsError {
    #[error(
        "GitHub repository must be given as 'OWNER/REPO', but given value was '{0}'"
    )]
    InvalidRepository(String),
}

pub async fn spr() -> Result<()> {
    let cli = Cli::parse();

    if let Commands::Init = cli.command {
        return commands::init::init().await;
    }

    // Discover the Jujutsu repository and get the colocated Git repo
    let current_dir = std::env::current_dir()?;
    let repo = git2::Repository::discover(&current_dir)?;
    
    // Verify this is a Jujutsu repository by checking for .jj directory
    let jj_dir = current_dir.join(".jj");
    if !jj_dir.exists() {
        return Err(Error::new(
            "This command requires a Jujutsu repository. Run 'jj git init --colocate' to create one.".to_string()
        ));
    }

    let git_config = repo.config()?;

    // Try to get config from jj first, fall back to git config
    let github_repository = match cli.github_repository {
        Some(v) => Ok(v),
        None => {
            // Try jj config first
            if let Ok(output) = std::process::Command::new("jj")
                .args(["config", "get", "spr.githubRepository"])
                .output()
            {
                if output.status.success() {
                    Ok(String::from_utf8(output.stdout)?.trim().to_string())
                } else {
                    git_config.get_string("spr.githubRepository")
                }
            } else {
                git_config.get_string("spr.githubRepository")
            }
        }
    }?;

    let (github_owner, github_repo) = {
        let captures = lazy_regex::regex!(r#"^([\w\-\.]+)/([\w\-\.]+)$"#)
            .captures(&github_repository)
            .ok_or_else(|| {
                OptionsError::InvalidRepository(github_repository.clone())
            })?;
        (
            captures.get(1).unwrap().as_str().to_string(),
            captures.get(2).unwrap().as_str().to_string(),
        )
    };

    let github_remote_name = get_config_value("spr.githubRemoteName", &git_config)
        .unwrap_or_else(|| "origin".to_string());
    let github_master_branch = get_config_value("spr.githubMasterBranch", &git_config)
        .unwrap_or_else(|| "main".to_string());
    let branch_prefix = get_config_value("spr.branchPrefix", &git_config)
        .ok_or_else(|| Error::new("spr.branchPrefix must be configured".to_string()))?;
    let require_approval = get_config_bool("spr.requireApproval", &git_config)
        .unwrap_or(false);
    let require_test_plan = get_config_bool("spr.requireTestPlan", &git_config)
        .unwrap_or(true);

    let config = jj_spr::config::Config::new(
        github_owner,
        github_repo,
        github_remote_name,
        github_master_branch,
        branch_prefix,
        require_approval,
        require_test_plan,
    );

    let jj = jj_spr::jj::Jujutsu::new(repo)
        .context("could not initialize Jujutsu backend".to_owned())?;

    if let Commands::Format(opts) = cli.command {
        return commands::format::format(opts, &jj, &config).await;
    }

    let github_auth_token = match cli.github_auth_token {
        Some(v) => v,
        None => get_config_value("spr.githubAuthToken", &git_config)
            .ok_or_else(|| Error::new("GitHub auth token must be configured".to_string()))?,
    };

    octocrab::initialise(
        octocrab::Octocrab::builder().personal_token(github_auth_token.clone()),
    )?;

    let mut headers = header::HeaderMap::new();
    headers.insert(header::ACCEPT, "application/json".parse()?);
    headers.insert(
        header::USER_AGENT,
        format!("spr/{}", env!("CARGO_PKG_VERSION")).try_into()?,
    );
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {}", github_auth_token).parse()?,
    );

    let graphql_client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;

    let mut gh = jj_spr::github::GitHub::new(
        config.clone(),
        graphql_client.clone(),
    );

    match cli.command {
        Commands::Diff(opts) => {
            commands::diff::diff(opts, &jj, &mut gh, &config).await?
        }
        Commands::Land(opts) => {
            commands::land::land(opts, &jj, &mut gh, &config).await?
        }
        Commands::Amend(opts) => {
            commands::amend::amend(opts, &jj, &mut gh, &config).await?
        }
        Commands::List => commands::list::list(graphql_client, &config).await?,
        Commands::Patch(opts) => {
            commands::patch::patch(opts, &jj, &mut gh, &config).await?
        }
        Commands::Close(opts) => {
            commands::close::close(opts, &jj, &mut gh, &config).await?
        }
        // The following commands are executed above and return from this
        // function before it reaches this match.
        Commands::Init | Commands::Format(_) => (),
    };

    Ok::<_, Error>(())
}

#[tokio::main]
async fn main() -> Result<()> {
    if let Err(error) = spr().await {
        for message in error.messages() {
            output("🛑", message)?;
        }
        std::process::exit(1);
    }

    Ok(())
}
