# jj-spr Documentation

jj-spr is a command line tool for using a stacked-diff workflow with GitHub, built for [Jujutsu](https://github.com/martinvonz/jj) version control.

The idea behind jj-spr is that your local branch management should not be dictated by your code-review tool. You should be able to send out code for review in individual commits, not branches. With Jujutsu's anonymous heads and stable change IDs, this workflow becomes even more natural - you don't need branches at all.

If you've used Phabricator's `arc` or the original `spr` tool, you'll find jj-spr very familiar, but enhanced with Jujutsu's powerful features.

## Table of Contents

### Getting Started
- [Installation](./user/installation.md)
- [Set up spr](./user/setup.md)

### How To
- [Create and Land a Simple PR](./user/simple.md)
- [Stack Multiple PRs](./user/stack.md)
- [Format and Update Commit Messages](./user/commit-message.md)
- [Check Out Someone Else's PR](./user/patch.md)

### Reference Guide
- [Configuration](./reference/configuration.md)

## Workflow overview

In jj-spr's workflow, you send out individual changes for review, not entire branches. This is the most basic version:

1. Create a new change on top of `main@origin` using `jj new`.

2. Run `jj-spr diff` to send out your change for review on GitHub.

3. If you need to make updates in response to feedback, simply edit your files - Jujutsu automatically absorbs the changes. Run `jj-spr diff` again to send those updates to GitHub.

   Similarly, you can rebase onto newer upstream `main` and run `jj-spr diff` to reflect any resulting changes to your commit.

4. Once reviewers have approved, run `jj-spr land`. This will put your change on top of the latest `main` and push it upstream.

In practice, you're likely to have more complex situations: multiple changes being reviewed, and possibly in-review changes that depend on others. You may need to make updates to any of these changes, or land them in any order.

jj-spr can handle all of that, leveraging Jujutsu's powerful features like stable change IDs and automatic rebasing. See the guides in the "How To" section for instructions on using jj-spr in those situations:

- [Simple PRs](./user/simple.md): no more than one review in flight on any branch.
- [Stacked PRs](./user/stack.md): multiple reviews in flight at once on your local `main`.

## Rationale

The reason to use jj-spr is that it perfectly aligns with Jujutsu's philosophy: you work with changes, not branches. Jujutsu's anonymous heads mean you never need to create branches for your work. Combined with stable change IDs that survive rebasing and amending, this creates an ideal environment for stacked diffs.

With Jujutsu + jj-spr:
- No branch management overhead - work directly with changes
- Stable change IDs make it easy to track and update specific changes in a stack
- Automatic rebasing keeps your entire stack up-to-date
- Conflicts are tracked as first-class objects, making complex rebases manageable

You can still create bookmarks (Jujutsu's equivalent of branches) if you want, but they're optional. The tool embraces Jujutsu's model where every change is automatically tracked and can be referenced by its stable ID.

### Why Review Changes?

The principle behind jj-spr is **one change per logical unit of work**. Each change should be able to stand on its own: it should have a coherent thesis and be a complete change in and of itself. It should have a clear summary, description, and test plan. It should leave the codebase in a consistent state: building and passing tests, etc.

In addition, ideally, it shouldn't be possible to further split a change into multiple changes that each stand on their own. If you _can_ split a change that way, you should (and Jujutsu's `jj split` makes this trivial).

What follows from those principles is the idea that **changes, not branches, should be the unit of code review**. The above description of a change also describes the ideal code review: a single, well-described change that leaves the codebase in a consistent state, and that cannot be subdivided further.

Jujutsu's model makes this natural: every change has a stable ID, can be individually addressed and modified, and maintains its identity through rebases. Why should the code review tool require branches when the VCS doesn't?

Following the one-change-per-review principle maintains the invariant that any change on `main` represents a codebase that has been reviewed _in that state_, and that builds and passes tests, etc. This makes it easy to revert changes, and to bisect.

[^master]: Git's default branch name is `master`, but GitHub's is now `main`, so we'll use `main` throughout this documentation.