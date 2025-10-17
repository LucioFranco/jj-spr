# Create and Land a Simple PR

This section details the process of putting a single commit up for review, and landing it (pushing it upstream). It assumes you don't have multiple reviews in flight at the same time. That situation is covered in [another guide](./stack.md), but you should be familiar with this single-review workflow before reading that one.

## Basic Workflow with Jujutsu

1. Fetch the latest changes from upstream:
   ```shell
   jj git fetch
   ```

2. Create a new change for your work:
   ```shell
   jj new main@origin -m "Add new feature"
   ```
   
   Or if you prefer working on an anonymous head:
   ```shell
   jj new main@origin
   ```

3. Make your changes and describe them:
   ```shell
   # Edit your files...
   jj describe -m "Add user authentication

   This implements basic user authentication using JWT tokens.
   
   Test Plan:
   - Added unit tests for auth module
   - Manually tested login/logout flow"
   ```

   See [this guide](./commit-message.md) for what to put in your commit message.

4. Run `jj-spr diff` to create a PR for your current change:
   ```shell
   jj-spr diff
   ```

5. Wait for reviewers to approve. If you need to make changes:

   1. Make whatever changes you need in your working copy.
   2. The changes are automatically absorbed into your current change (this is Jujutsu's default behavior).
   3. Update the description if needed:
      ```shell
      jj describe
      ```
   4. Run `jj-spr diff` again. If you changed the commit message, you will need to add the flag `--update-message`:
      ```shell
      jj-spr diff --update-message
      ```

      This will update the PR with the new version of your change. jj-spr will prompt you for a short message that describes what you changed. You can also pass the update message on the command line using the `--message`/`-m` flag.

6. Once your PR is approved, run `jj-spr land` to push it upstream:
   ```shell
   jj-spr land
   ```

## Working with Change IDs

In Jujutsu, every change has a stable change ID (like `qpvuntsm`). You can use these IDs to refer to specific changes:

```shell
# Create a PR for a specific change
jj-spr diff -r qpvuntsm

# Land a specific change
jj-spr land -r qpvuntsm
```

## When you update

When you run `jj-spr diff` to update an existing PR, your update will be added to the PR as a new commit, so that reviewers can see exactly what changed. The new commit's message will be what you entered when prompted.

The individual commits that you see in the PR are solely for the benefit of reviewers; they will not be reflected in the commit history when the PR is landed. The commit that eventually lands on upstream `main` will always be a single commit, whose message is the title and description from the PR.

## Updating before landing

Unlike Git, Jujutsu automatically maintains your change's identity even when rebasing. However, you must still run `jj-spr diff` to update the PR before landing if you've rebased onto new upstream changes, or else `jj-spr land` will fail.

This is because `jj-spr land` checks to make sure that the PR content matches what will be landed.

## Conflicts on landing

`jj-spr land` may fail with conflicts if there have been new changes pushed to upstream `main` since you last fetched. In this case:

1. Fetch and rebase your change onto latest upstream `main`:
   ```shell
   jj git fetch
   jj rebase -r @ -d main@origin
   ```

2. Resolve any conflicts:
   ```shell
   # Jujutsu will mark conflicts in the files
   # Edit the files to resolve conflicts
   jj resolve
   ```

3. Run `jj-spr diff` to update the PR:
   ```shell
   jj-spr diff
   ```

4. Run `jj-spr land` again:
   ```shell
   jj-spr land
   ```

Note that even if your change is not based on the latest upstream `main`, landing will still succeed as long as there are no conflicts with the actual latest upstream `main`.