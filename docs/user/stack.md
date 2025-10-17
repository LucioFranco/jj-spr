# Stack Multiple PRs

The differences between jj-spr's commit-based workflow and GitHub's default branch-based workflow are most apparent when you have multiple reviews in flight at the same time.

This guide assumes you're already familiar with the workflow for [simple, non-stacked PRs](./simple.md).

In Jujutsu, managing stacked changes is much simpler than in Git because Jujutsu maintains stable change IDs and automatically handles rebasing operations.

## Creating a Stack of Changes

This is the workflow for creating multiple PRs at the same time. This example only creates two, but the workflow works for arbitrarily deep stacks.

1. Create your first change on top of `main`:
   ```shell
   jj new main@origin -m "Add authentication module"
   # Make changes...
   ```

2. Create your second change on top of the first:
   ```shell
   jj new -m "Add user profile endpoints"
   # Make changes that depend on the authentication module...
   ```

3. Run `jj-spr diff --all` to create PRs for all changes in your stack:
   ```shell
   jj-spr diff --all
   ```
   
   This is equivalent to calling `jj-spr diff` on each change in your stack from the current change back to `main@origin`.

## Understanding Your Stack

Use `jj log` to visualize your stack:
```shell
jj log -r 'main@origin..'
```

This shows all your changes that are descendants of `main@origin`.

## Updating Changes in the Stack

Suppose you need to update the first change (authentication module) in response to review feedback:

1. Edit the first change directly:
   ```shell
   jj edit <change-id-of-first-change>
   # Make your changes...
   ```

2. The changes are automatically absorbed. Jujutsu will automatically rebase any descendant changes.

3. Update the PR for that specific change:
   ```shell
   jj-spr diff -r @
   ```

4. Return to where you were working:
   ```shell
   jj edit <previous-change-id>
   ```

Alternatively, you can use `jj squash` to move changes down the stack:

1. Make your changes on top of the stack:
   ```shell
   jj new -m "fixes for auth module"
   # Make changes...
   ```

2. Squash the changes into the target change:
   ```shell
   jj squash --into <change-id-of-auth-module>
   ```

3. Update the PR:
   ```shell
   jj-spr diff -r <change-id-of-auth-module>
   ```

## Landing Stacked Changes

By default, you must land changes in order (parent before child). To land the first change in your stack:

1. Run `jj-spr land` on the specific change:
   ```shell
   jj-spr land -r <change-id-of-first-change>
   ```

2. After landing, fetch the latest changes:
   ```shell
   jj git fetch
   ```

3. Your remaining changes are automatically rebased by Jujutsu.

4. Update the PRs for the remaining changes:
   ```shell
   jj-spr diff --all
   ```

## Cherry-picking for Independent Changes

If your changes are truly independent and you want to be able to land them in any order:

1. Create the first change as before:
   ```shell
   jj new main@origin -m "Add authentication module"
   # Make changes...
   jj-spr diff
   ```

2. Create the second change with the `--cherry-pick` flag:
   ```shell
   jj new -m "Add user profile endpoints"
   # Make changes...
   jj-spr diff --cherry-pick
   ```

   The `--cherry-pick` flag causes jj-spr to create the PR as if the change were based directly on `main@origin`, rather than creating a synthetic base branch.

3. Once either change is ready to land, you can land it with:
   ```shell
   jj-spr land --cherry-pick -r <change-id>
   ```

   The `--cherry-pick` flag is required because by default, `jj-spr land` refuses to land a change whose parent is not on upstream `main`.

## Rebasing the Whole Stack

One of the major advantages of Jujutsu is that rebasing your entire stack onto new upstream changes is trivial:

1. Fetch the latest changes:
   ```shell
   jj git fetch
   ```

2. Rebase your stack:
   ```shell
   jj rebase -s <root-change-id> -d main@origin
   ```
   
   Where `<root-change-id>` is the first change in your stack.

3. Update all PRs:
   ```shell
   jj-spr diff --all
   ```

## Working with Revsets

Jujutsu's revset language makes it easy to work with stacks:

```shell
# Show all your changes not yet in main
jj log -r 'mine() & ~main@origin'

# Create PRs for all your ready changes
jj-spr diff --all -r 'ready() & ~main@origin'

# Show changes that have PRs
jj log -r 'description(regex:"#[0-9]+")' 
```

## Tips for Stack Management

1. **Keep changes focused**: Each change should represent one logical unit of work.

2. **Use descriptive commit messages**: This helps when navigating your stack.

3. **Leverage change IDs**: Unlike Git commits, Jujutsu change IDs remain stable through rebases.

4. **Use `jj split` when needed**: If a change gets too large, split it:
   ```shell
   jj split -r <change-id>
   ```

5. **Monitor your stack**: Regularly run `jj log` to understand your stack's structure.

The Jujutsu + jj-spr workflow makes stacked PRs feel natural and eliminates much of the complexity found in traditional Git-based stacking workflows.