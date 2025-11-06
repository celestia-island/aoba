//! Retry state machine for step group execution
//!
//! Groups consecutive keyboard + verification steps into retryable units.
//! On verification failure, retries the entire group up to 3 times.

use anyhow::Result;

use crate::workflow::WorkflowStep;

/// State machine states for retry logic
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryState {
    /// Scanning steps to identify groups
    Scanning,
    /// Executing action steps in current group
    Executing,
    /// Verifying results of action steps
    Verifying,
    /// Retrying after verification failure
    Retrying,
    /// Group execution succeeded
    Success,
    /// Group execution failed after max retries
    Failed,
}

/// A group of steps that can be retried as a unit
#[derive(Debug, Clone)]
pub struct StepGroup {
    /// Indices of steps in this group
    pub step_indices: Vec<usize>,
    /// Current retry attempt (0 = first attempt)
    pub retry_count: u32,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Cached screenshot from first failure
    pub first_failure_screenshot: Option<String>,
}

impl StepGroup {
    pub fn new(step_indices: Vec<usize>) -> Self {
        Self {
            step_indices,
            retry_count: 0,
            max_retries: 3,
            first_failure_screenshot: None,
        }
    }

    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }
}

/// Identifies if a step is an action (keyboard/input) step
pub fn is_action_step(step: &WorkflowStep) -> bool {
    step.key.is_some() || step.input.is_some()
}

/// Identifies if a step is a verification step
pub fn is_verification_step(step: &WorkflowStep) -> bool {
    step.verify.is_some() || step.mock_verify_path.is_some()
}

/// Identifies if a step is a mock modification step (doesn't break groups)
pub fn is_mock_modification_step(step: &WorkflowStep) -> bool {
    step.mock_path.is_some() && step.mock_set_value.is_some()
}

/// Identifies if a step breaks the group (trigger, large sleep)
pub fn breaks_group(step: &WorkflowStep) -> bool {
    // Triggers break groups
    if step.trigger.is_some() {
        return true;
    }
    // Large sleeps (>1000ms) break groups
    if let Some(sleep_ms) = step.sleep_ms {
        if sleep_ms > 1000 {
            return true;
        }
    }
    false
}

/// Group consecutive steps into retryable units
///
/// A retryable group consists of:
/// - One or more action steps (keyboard/input)
/// - One or more verification steps
/// - Optionally mock modification steps (which don't break the group)
pub fn group_steps(steps: &[WorkflowStep]) -> Vec<StepGroup> {
    let mut groups = Vec::new();
    let mut current_indices = Vec::new();
    let mut has_actions = false;
    let mut has_verifications = false;

    for (i, step) in steps.iter().enumerate() {
        // Check if this step breaks the group
        if breaks_group(step) {
            // Finalize current group if it has both actions and verifications
            if !current_indices.is_empty() && has_actions && has_verifications {
                groups.push(StepGroup::new(current_indices.clone()));
            }
            current_indices.clear();
            has_actions = false;
            has_verifications = false;
            continue;
        }

        // Track step type
        if is_action_step(step) {
            has_actions = true;
            current_indices.push(i);
        } else if is_verification_step(step) {
            has_verifications = true;
            current_indices.push(i);
        } else if is_mock_modification_step(step) || step.sleep_ms.is_some() {
            // Mock modifications and small sleeps are part of the group
            current_indices.push(i);
        } else {
            // Other steps don't break groups but are included
            current_indices.push(i);
        }
    }

    // Finalize last group if valid
    if !current_indices.is_empty() && has_actions && has_verifications {
        groups.push(StepGroup::new(current_indices));
    }

    groups
}

/// Check if a step index is part of any retryable group
pub fn is_in_retryable_group(step_index: usize, groups: &[StepGroup]) -> bool {
    groups.iter().any(|g| g.step_indices.contains(&step_index))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_steps_basic() {
        let steps = vec![
            WorkflowStep {
                key: Some("Enter".to_string()),
                ..Default::default()
            },
            WorkflowStep {
                verify: Some("Expected".to_string()),
                ..Default::default()
            },
        ];

        let groups = group_steps(&steps);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].step_indices, vec![0, 1]);
    }

    #[test]
    fn test_mock_steps_dont_break_groups() {
        let steps = vec![
            WorkflowStep {
                key: Some("Enter".to_string()),
                ..Default::default()
            },
            WorkflowStep {
                mock_path: Some("test".to_string()),
                mock_set_value: Some(serde_json::json!(42)),
                ..Default::default()
            },
            WorkflowStep {
                verify: Some("Expected".to_string()),
                ..Default::default()
            },
        ];

        let groups = group_steps(&steps);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].step_indices, vec![0, 1, 2]);
    }

    #[test]
    fn test_triggers_break_groups() {
        let steps = vec![
            WorkflowStep {
                key: Some("Enter".to_string()),
                ..Default::default()
            },
            WorkflowStep {
                verify: Some("Expected".to_string()),
                ..Default::default()
            },
            WorkflowStep {
                trigger: Some("test_trigger".to_string()),
                ..Default::default()
            },
            WorkflowStep {
                key: Some("Down".to_string()),
                ..Default::default()
            },
            WorkflowStep {
                verify: Some("Another".to_string()),
                ..Default::default()
            },
        ];

        let groups = group_steps(&steps);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].step_indices, vec![0, 1]);
        assert_eq!(groups[1].step_indices, vec![3, 4]);
    }
}

impl Default for WorkflowStep {
    fn default() -> Self {
        Self {
            description: None,
            key: None,
            times: None,
            input: None,
            value: None,
            verify: None,
            at_line: None,
            verify_with_placeholder: None,
            cursor_at_line: None,
            sleep_ms: None,
            mock_path: None,
            mock_set_value: None,
            mock_set_value_with_placeholder: None,
            mock_verify_path: None,
            mock_verify_value: None,
            trigger: None,
            trigger_params: None,
        }
    }
}
