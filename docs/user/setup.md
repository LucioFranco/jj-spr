# Set up jj-spr

In the Jujutsu repo you want to use jj-spr in, run `jj-spr init`; this will ask you several questions.

You'll need to provide a GitHub personal access token (PAT) as the first step. [See the GitHub docs](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/creating-a-personal-access-token) on how to create one. `jj-spr init` will tell you which scopes the token must have; make sure to set them correctly when creating the token.

The rest of the settings that `jj-spr init` asks for have sensible defaults, so almost all users can simply accept the defaults. The most common situation where you would need to diverge from the defaults is if the remote representing GitHub is not called `origin`.

See the [Configuration](../reference/configuration.md) reference page for full details about the available settings.

## Jujutsu Configuration

jj-spr works best when your Jujutsu configuration is set up appropriately. Here's a recommended configuration:

```toml
# In your ~/.jjconfig.toml or .jj/repo/config.toml

[user]
name = "Your Name"
email = "your.email@example.com"

[ui]
default-command = "log"

# Configure Git integration
[git]
push-branch-prefix = "jj-spr/"

# Set up revsets for common workflows
[revset-aliases]
'mine' = 'author(exact:"your.email@example.com")'
'ready' = 'mine & description(regex:"^(?!wip:)")'
```

## Setting up jj-spr Configuration

After initial setup with `jj-spr init`, you can update your settings in several ways:

- Simply rerun `jj-spr init`. The defaults it suggests will be your existing settings, so you can easily change only what you need to.

- Use `jj config set` to modify Jujutsu configuration:
  ```shell
  jj config set --repo spr.githubAuthToken "your-token"
  jj config set --repo spr.requireTestPlan true
  ```

- Edit the `[spr]` section of `.jj/repo/config.toml` directly:
  ```toml
  [spr]
  githubAuthToken = "your-token"
  githubRepository = "owner/repo"
  branchPrefix = "jj-spr"
  requireApproval = true
  requireTestPlan = false
  ```

## Understanding Jujutsu's Anonymous Heads

One key difference when using jj-spr is understanding Jujutsu's concept of anonymous heads (commits without bookmarks/branches). In Jujutsu:

- You don't need to create branches for your work
- Commits are identified by their change IDs (e.g., `qpvuntsm`)
- jj-spr will create GitHub branches automatically when you run `jj-spr diff`
- The tool maintains stable commit identities even when rebasing

This means you can work directly on your changes without worrying about branch management - jj-spr handles the GitHub side for you.