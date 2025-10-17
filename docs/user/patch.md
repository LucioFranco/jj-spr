# Check Out Someone Else's PR

While reviewing someone else's pull request, it may be useful to pull their changes to your local repo, so you can run their code, or view it in your editor/IDE, etc.

## Fetching a PR with jj-spr

To check out someone else's PR, get the number of the PR you want to pull, and run:

```shell
jj-spr patch <number>
```

This command will:
1. Fetch the PR's commits from GitHub
2. Create a new change in your Jujutsu repo with the PR's content
3. Set you up to work with the PR locally

## Understanding the PR Structure in Jujutsu

When you fetch a PR with `jj-spr patch`, the tool:

- Creates a new change based on the PR's base commit
- If the PR has multiple commits, they are combined into a single change
- The change description includes the PR metadata

You can see the fetched PR in your log:
```shell
jj log -r ::@
```

## How jj-spr patch works

When fetching a PR created by someone else, the structure looks like this:

```
PR creator's repo:                Your repo after 'jj-spr patch':
                                  
X (runs spr diff here)            main (A) ──── X 
│                                          \
Y                                           └── Y + Z (squashed)
│                                               │
Z (PR #123)                                     @ (working copy)
│
main (A)
```

The fetched changes are organized as:
- The PR's base commit (X) is identified and checked out
- If there are intermediate commits (Y), they're squashed with the PR commit (Z)
- You get a clean view with at most two commits: the base and the PR changes

## Working with the Fetched PR

Once you have the PR locally, you can:

1. **Run and test the code**:
   ```shell
   # The PR changes are in your working copy
   cargo test
   cargo run
   ```

2. **Make local modifications**:
   ```shell
   # Any edits you make are automatically tracked
   # Edit files as needed...
   ```

3. **Compare with the base**:
   ```shell
   jj diff --from @-
   ```

## Updating Someone Else's PR

If you have permission to update the PR (e.g., you're a maintainer or collaborator), you can push changes back:

1. Make your changes in the working copy
2. Update the change description if needed:
   ```shell
   jj describe
   ```
3. Push the updates back to the PR:
   ```shell
   jj-spr diff --update-message
   ```

**Important**: This will overwrite the contents of the PR on GitHub with what you have locally. Always coordinate with the PR creator before doing so.

## Cleaning Up

After you're done reviewing, you can abandon the fetched change:

```shell
# Move to a different change first
jj new main@origin

# Then abandon the PR change
jj abandon <pr-change-id>
```

Or if you want to keep the change around for reference:
```shell
# Create a bookmark for future reference
jj bookmark create pr-<number> -r <pr-change-id>
```

## Tips for PR Review Workflow

1. **Use revsets to manage multiple PRs**:
   ```shell
   # Show all fetched PRs
   jj log -r 'description(regex:"PR #[0-9]+")' 
   ```

2. **Create a dedicated workspace** for PR reviews:
   ```shell
   jj workspace add ~/reviews
   cd ~/reviews
   jj-spr patch <number>
   ```

3. **Compare PRs with each other**:
   ```shell
   jj diff --from <pr1-change-id> --to <pr2-change-id>
   ```

The Jujutsu workflow makes it easy to work with multiple PRs simultaneously without the branch management overhead of Git.