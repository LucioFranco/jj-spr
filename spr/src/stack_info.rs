/*
 * Copyright (c) Radical HQ Limited
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::{
    config::Config,
    message::{MessageSection, MessageSectionsMap},
};

/// Represents information about a PR's position in a stack
#[derive(Debug, Clone)]
pub struct StackPosition {
    /// Current position (1-indexed)
    pub current: usize,
    /// Total number of PRs in stack
    pub total: usize,
    /// PR number of the parent (depends on)
    pub parent_pr: Option<u64>,
    /// PR numbers of children (required for)
    pub child_prs: Vec<u64>,
}

/// Generate stack information text for a PR description
pub fn build_stack_info_text(
    position: &StackPosition,
    config: &Config,
    all_commits: &[(Option<u64>, MessageSectionsMap)],
) -> String {
    let mut text = String::new();

    // Add horizontal rule separator
    text.push_str("---\n");

    // Add stack position
    text.push_str(&format!(
        "**Stack Position: {} of {}**\n\n",
        position.current, position.total
    ));

    // Add parent PR link if exists
    if let Some(parent_pr) = position.parent_pr {
        let parent_title = all_commits
            .iter()
            .find(|(pr, _)| *pr == Some(parent_pr))
            .and_then(|(_, msg)| msg.get(&MessageSection::Title))
            .map(|t| format!(" - {}", t))
            .unwrap_or_default();

        text.push_str(&format!(
            "⬆️ **Depends on:** {}/{}#{}{}\n",
            config.owner, config.repo, parent_pr, parent_title
        ));
    }

    // Add child PR links if exist
    if !position.child_prs.is_empty() {
        text.push_str("⬇️ **Required for:**");
        for child_pr in &position.child_prs {
            let child_title = all_commits
                .iter()
                .find(|(pr, _)| *pr == Some(*child_pr))
                .and_then(|(_, msg)| msg.get(&MessageSection::Title))
                .map(|t| format!(" - {}", t))
                .unwrap_or_default();

            text.push_str(&format!(
                " {}/{}#{}{}",
                config.owner, config.repo, child_pr, child_title
            ));
        }
        text.push('\n');
    }

    // Add full stack visualization if stack has more than 1 PR
    if position.total > 1 {
        text.push_str("\n**Full Stack:**\n");

        for (idx, (pr_num_opt, message)) in all_commits.iter().enumerate() {
            if let Some(pr_num) = pr_num_opt {
                let num = idx + 1;
                let title = message
                    .get(&MessageSection::Title)
                    .map(|t| format!(" - {}", t))
                    .unwrap_or_default();

                let indicator = if num == position.current {
                    " (this PR)"
                } else {
                    ""
                };

                text.push_str(&format!(
                    "{}. {}/{}#{}{}{}\n",
                    num, config.owner, config.repo, pr_num, title, indicator
                ));
            }
        }
    }

    text.push_str("\n---");

    text
}

/// Detect stack position for a commit within a list of commits
pub fn detect_stack_position(
    current_index: usize,
    all_commits: &[(Option<u64>, MessageSectionsMap)],
) -> Option<StackPosition> {
    // Only generate stack info if there are multiple commits with PR numbers
    let commits_with_prs: Vec<(usize, u64)> = all_commits
        .iter()
        .enumerate()
        .filter_map(|(idx, (pr_opt, _))| pr_opt.map(|pr| (idx, pr)))
        .collect();

    if commits_with_prs.len() <= 1 {
        return None;
    }

    // Find the current commit's position in the stack
    let stack_position = commits_with_prs
        .iter()
        .position(|(idx, _)| *idx == current_index)?;

    // Get parent PR (previous in stack)
    let parent_pr = if stack_position > 0 {
        Some(commits_with_prs[stack_position - 1].1)
    } else {
        None
    };

    // Get child PRs (all subsequent in stack)
    let child_prs: Vec<u64> = commits_with_prs
        .iter()
        .skip(stack_position + 1)
        .map(|(_, pr)| *pr)
        .collect();

    Some(StackPosition {
        current: stack_position + 1,
        total: commits_with_prs.len(),
        parent_pr,
        child_prs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn create_test_commit_snapshot(
        pr_number: Option<u64>,
        title: &str,
    ) -> (Option<u64>, MessageSectionsMap) {
        let mut message = BTreeMap::new();
        message.insert(MessageSection::Title, title.to_string());
        (pr_number, message)
    }

    #[test]
    fn test_detect_stack_position_single_commit() {
        let commits = vec![create_test_commit_snapshot(Some(1), "Test PR")];

        let position = detect_stack_position(0, &commits);
        assert!(
            position.is_none(),
            "Single commit should not have stack info"
        );
    }

    #[test]
    fn test_detect_stack_position_first_of_two() {
        let commits = vec![
            create_test_commit_snapshot(Some(1), "First PR"),
            create_test_commit_snapshot(Some(2), "Second PR"),
        ];

        let position = detect_stack_position(0, &commits).unwrap();
        assert_eq!(position.current, 1);
        assert_eq!(position.total, 2);
        assert_eq!(position.parent_pr, None);
        assert_eq!(position.child_prs, vec![2]);
    }

    #[test]
    fn test_detect_stack_position_second_of_two() {
        let commits = vec![
            create_test_commit_snapshot(Some(1), "First PR"),
            create_test_commit_snapshot(Some(2), "Second PR"),
        ];

        let position = detect_stack_position(1, &commits).unwrap();
        assert_eq!(position.current, 2);
        assert_eq!(position.total, 2);
        assert_eq!(position.parent_pr, Some(1));
        assert_eq!(position.child_prs, Vec::<u64>::new());
    }

    #[test]
    fn test_detect_stack_position_middle_of_three() {
        let commits = vec![
            create_test_commit_snapshot(Some(1), "First PR"),
            create_test_commit_snapshot(Some(2), "Second PR"),
            create_test_commit_snapshot(Some(3), "Third PR"),
        ];

        let position = detect_stack_position(1, &commits).unwrap();
        assert_eq!(position.current, 2);
        assert_eq!(position.total, 3);
        assert_eq!(position.parent_pr, Some(1));
        assert_eq!(position.child_prs, vec![3]);
    }

    #[test]
    fn test_detect_stack_position_with_missing_pr() {
        let commits = vec![
            create_test_commit_snapshot(Some(1), "First PR"),
            create_test_commit_snapshot(None, "Not submitted yet"),
            create_test_commit_snapshot(Some(2), "Third PR"),
        ];

        // First commit should see only one other PR
        let position = detect_stack_position(0, &commits).unwrap();
        assert_eq!(position.current, 1);
        assert_eq!(position.total, 2);
        assert_eq!(position.parent_pr, None);
        assert_eq!(position.child_prs, vec![2]);

        // Middle commit has no PR, so no stack position
        let position = detect_stack_position(1, &commits);
        assert!(position.is_none());

        // Third commit should see first as parent
        let position = detect_stack_position(2, &commits).unwrap();
        assert_eq!(position.current, 2);
        assert_eq!(position.total, 2);
        assert_eq!(position.parent_pr, Some(1));
        assert_eq!(position.child_prs, Vec::<u64>::new());
    }

    #[test]
    fn test_build_stack_info_text_format() {
        let commits = vec![
            create_test_commit_snapshot(Some(120), "Add authentication module"),
            create_test_commit_snapshot(Some(121), "Add user session handling"),
            create_test_commit_snapshot(Some(122), "Add user profile endpoints"),
        ];

        let position = StackPosition {
            current: 2,
            total: 3,
            parent_pr: Some(120),
            child_prs: vec![122],
        };

        let config = Config::new(
            "LucioFranco".to_string(),
            "jj-spr".to_string(),
            "origin".to_string(),
            "main".to_string(),
            "spr/".to_string(),
            false,
            false,
        );

        let text = build_stack_info_text(&position, &config, &commits);

        // Check key elements are present
        assert!(text.contains("Stack Position: 2 of 3"));
        assert!(text.contains("⬆️ **Depends on:** LucioFranco/jj-spr#120"));
        assert!(text.contains("Add authentication module"));
        assert!(text.contains("⬇️ **Required for:** LucioFranco/jj-spr#122"));
        assert!(text.contains("Add user profile endpoints"));
        assert!(text.contains("**Full Stack:**"));
        assert!(text.contains("(this PR)"));
        assert!(text.starts_with("---"));
        assert!(text.ends_with("---"));
    }
}
