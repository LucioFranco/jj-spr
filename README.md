# jj-spr

A command-line tool for submitting and updating GitHub Pull Requests from local Jujutsu commits that may be amended and rebased. Pull Requests can be stacked to allow for a series of code reviews of interdependent code.

Designed to be run as a Jujutsu subcommand: `jj spr <command>`.

## About This Fork

This project is a fork of the original [spr](https://github.com/getcord/spr) tool, specifically building upon the Jujutsu integration work started by [sunshowers](https://github.com/sunshowers). 

### Why This Fork Exists

This fork is a fork of a fork of [spr](https://github.com/spacedentist/spr) which was then forked by [`sunshowers`](https://github.com/sunshowers/spr).

This fork continues the sunshowers fork but tries to integrate fully with jujutsu rather than be based fully on git with a few jj parts.

## Installation

### From Source

```bash
git clone https://github.com/your-username/jj-spr.git
cd jj-spr
cargo install --path spr
```

This will install the `jj-spr` binary to your `~/.cargo/bin` directory.

### Setting Up as a Jujutsu Subcommand

To use `jj-spr` as a Jujutsu subcommand (enabling `jj spr <command>`), add the following to your Jujutsu configuration:

#### Option 1: Using `jj config set` (Recommended)

```bash
jj config set --user aliases.spr '["util", "exec", "--", "jj-spr"]'
```

#### Option 2: Manual Configuration

Add this to your Jujutsu config file (`~/.jjconfig.toml` or `.jj/repo/config.toml`):

```toml
[aliases]
spr = ["util", "exec", "--", "jj-spr"]
```

#### Option 3: Direct Binary Configuration

If you prefer to configure the binary path directly:

```toml
[aliases]
spr = ["util", "exec", "--", "/path/to/jj-spr"]
```

After configuration, you can use commands like:
```bash
jj spr diff           # Create/update a PR for the current change
jj spr diff -r @~1     # Create/update a PR for a specific change
jj spr land            # Land (merge) a PR
jj spr list            # List open PRs
```

## Quickstart

### Initial Setup

1. **Initialize in your repository:**
   ```bash
   cd your-jujutsu-repo
   jj spr init
   ```

2. **Provide your GitHub Personal Access Token** when prompted. This allows jj-spr to create and manage pull requests via the GitHub API.

### Basic Workflow

1. **Create a commit:**
   ```bash
   echo "new feature" > feature.txt
   jj commit -m "Add new feature"
   ```

2. **Submit for review:**
   ```bash
   jj spr diff
   ```

3. **Make changes and update:**
   ```bash
   echo "updated feature" > feature.txt
   jj describe -m "Add new feature (updated)"
   jj spr diff -m "Updated implementation"
   ```

4. **Land the PR:**
   ```bash
   jj spr land
   ```

### Stacked Pull Requests

jj-spr excels at handling stacked PRs for related changes:

```bash
# Create first change
jj commit -m "Foundation change"
jj spr diff  # Creates PR #1

# Create dependent change
jj commit -m "Building on foundation"  
jj spr diff  # Creates PR #2 stacked on PR #1

# Update just the second change
jj spr diff -r @  # Updates only PR #2
```

## Commands

### Core Commands

- **`jj spr diff`** - Create or update a pull request for the current change
- **`jj spr land`** - Land (squash-merge) an approved pull request
- **`jj spr list`** - List open pull requests and their status
- **`jj spr close`** - Close a pull request
- **`jj spr amend`** - Update local commit message with content from GitHub

### Command Options

Most commands support revision selection:
```bash
jj spr diff -r @-            # Specific revision
jj spr diff -r main..@       # Range of revisions (like --all)
jj spr diff -a --base trunk  # All changes from trunk to current
```

For detailed help on any command:
```bash
jj spr help <command>
```

## Configuration

jj-spr stores configuration in your repository's git config:

```bash
# Set GitHub repository (if not auto-detected)
git config spr.githubRepository "owner/repo"

# Set branch prefix for generated branches
git config spr.branchPrefix "yourname/spr/"

# Require approval before landing
git config spr.requireApproval true

# Require test plan in commit messages
git config spr.requireTestPlan true
```

## Requirements

- **Jujutsu**: A working Jujutsu installation with a colocated Git repository
- **GitHub Access**: A GitHub Personal Access Token with appropriate permissions
- **Git Repository**: Your Jujutsu repository must be colocated with Git (`jj git init --colocate`)

## Differences from Original spr

### Jujutsu-Specific Features

- **Change ID Handling**: Works with Jujutsu's change IDs instead of Git commit hashes
- **Commit Identity Preservation**: Uses `jj describe` to maintain commit identity when updating messages
- **Native Jujutsu Commands**: Integrates with `jj log`, `jj commit`, and other Jujutsu operations

### Enhanced Stacking Support

- **Fixed Parent Immutability**: Stacked changes no longer make parent commits immutable
- **Proper Base Branches**: Correctly creates base branches for stacked PRs
- **Clean Diffs**: GitHub shows only child changes, not cumulative diffs

### Improved CLI Experience

- **Per-Command Revisions**: `jj spr diff -r <rev>` instead of global revision flags
- **Range Support**: `jj spr diff -r main..@` automatically enables multi-commit mode
- **Better Defaults**: Uses `@-` (parent of working copy) as the default revision

## Contributing

Contributions are welcome! Please:

1. **Check existing issues** before starting work
2. **Add tests** for new functionality
3. **Follow the existing code style** (run `cargo fmt` and `cargo clippy`)
4. **Update documentation** as needed

### Running Tests

```bash
# Run unit tests
cargo test

# Run integration tests (requires jj and git)
cargo test --test '*'

# Check code quality
cargo clippy --all-features --all-targets
cargo fmt --check
```

## License

This project is MIT licensed. See [LICENSE](./LICENSE) for details.

## Credits

- Original [spr](https://github.com/getcord/spr) by the Cord team
- Jujutsu integration foundation by [sunshowers](https://github.com/sunshowers)
- Jujutsu project by [martinvonz](https://github.com/martinvonz) and contributors
