//! Retry state machine for step group execution
//!
//! Groups consecutive keyboard + verification steps into retryable units.
//! On verification failure, retries the entire group up to 3 times.

use crate::workflow::WorkflowStep;

/// A group of steps that can be retried as a unit
#[derive(Debug, Clone)]
pub struct StepGroup {
    /// Indices of steps in this group (in execution order)
    pub step_indices: Vec<usize>,
    /// Indices of action-oriented steps inside this group
    pub action_indices: Vec<usize>,
    /// Maximum retry attempts
    pub max_retries: u32,
}

impl StepGroup {
    pub fn new(step_indices: Vec<usize>, action_indices: Vec<usize>) -> Self {
        Self {
            step_indices,
            action_indices,
            max_retries: 3,
        }
    }
}

#[derive(Debug, Clone)]
struct GroupBuilder {
    step_indices: Vec<usize>,
    action_indices: Vec<usize>,
    verification_indices: Vec<usize>,
    stage: GroupStage,
}

impl GroupBuilder {
    fn new() -> Self {
        Self {
            step_indices: Vec::new(),
            action_indices: Vec::new(),
            verification_indices: Vec::new(),
            stage: GroupStage::Actions,
        }
    }

    fn push_action(&mut self, index: usize) {
        self.step_indices.push(index);
        self.action_indices.push(index);
    }

    fn push_verification(&mut self, index: usize) {
        self.step_indices.push(index);
        self.verification_indices.push(index);
    }

    fn push_support(&mut self, index: usize) {
        self.step_indices.push(index);
    }

    fn start_verifications(&mut self) {
        self.stage = GroupStage::Verifications;
    }

    fn has_actions(&self) -> bool {
        !self.action_indices.is_empty()
    }

    fn has_verifications(&self) -> bool {
        !self.verification_indices.is_empty()
    }

    fn finish(self) -> Option<StepGroup> {
        if self.has_actions() && self.has_verifications() {
            Some(StepGroup::new(self.step_indices, self.action_indices))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupStage {
    Actions,
    Verifications,
}

/// Identifies if a step is an action (keyboard/input) step
pub fn is_action_step(step: &WorkflowStep) -> bool {
    step.key.is_some() || step.input.is_some()
}

/// Identifies if a step is a verification step
pub fn is_verification_step(step: &WorkflowStep) -> bool {
    step.verify.is_some()
        || step.verify_with_placeholder.is_some()
        || step.cursor_at_line.is_some()
        || step.mock_verify_path.is_some()
}

/// Identifies if a step is a mock modification step (doesn't break groups)
pub fn is_mock_modification_step(step: &WorkflowStep) -> bool {
    step.mock_path.is_some()
        && (step.mock_set_value.is_some() || step.mock_set_value_with_placeholder.is_some())
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
    let mut builder: Option<GroupBuilder> = None;

    for (index, step) in steps.iter().enumerate() {
        if breaks_group(step) {
            if let Some(group) = builder.take().and_then(|b| b.finish()) {
                groups.push(group);
            }
            continue;
        }

        let is_action = is_action_step(step);
        let is_verification = is_verification_step(step);
        let is_support =
            is_mock_modification_step(step) || matches!(step.sleep_ms, Some(ms) if ms <= 1000);

        if builder.is_none() {
            if is_action {
                let mut new_builder = GroupBuilder::new();
                new_builder.push_action(index);
                builder = Some(new_builder);
            }
            continue;
        }

        let mut current = builder.take().unwrap();

        if is_action {
            if current.stage == GroupStage::Verifications && current.has_verifications() {
                if let Some(group) = current.finish() {
                    groups.push(group);
                }
                let mut new_builder = GroupBuilder::new();
                new_builder.push_action(index);
                builder = Some(new_builder);
            } else {
                current.push_action(index);
                builder = Some(current);
            }
        } else if is_verification {
            if !current.has_actions() {
                // Ignore stray verification without actions
                builder = Some(current);
                continue;
            }

            if current.stage == GroupStage::Actions {
                current.start_verifications();
            }
            current.push_verification(index);
            builder = Some(current);
        } else if is_support {
            if current.has_actions() {
                current.push_support(index);
            }
            builder = Some(current);
        } else {
            if let Some(group) = current.finish() {
                groups.push(group);
            }
            builder = None;
        }
    }

    if let Some(group) = builder.and_then(|b| b.finish()) {
        groups.push(group);
    }

    groups
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

// Note: helper `is_in_retryable_group` was removed because it was not used elsewhere.

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
        assert_eq!(groups[0].action_indices, vec![0]);
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
        assert_eq!(groups[0].action_indices, vec![0]);
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
        assert_eq!(groups[0].action_indices, vec![0]);
        assert_eq!(groups[1].step_indices, vec![3, 4]);
        assert_eq!(groups[1].action_indices, vec![3]);
    }

    #[test]
    fn test_consecutive_actions_and_verifications() {
        let steps = vec![
            WorkflowStep {
                key: Some("Enter".to_string()),
                ..Default::default()
            },
            WorkflowStep {
                key: Some("Down".to_string()),
                ..Default::default()
            },
            WorkflowStep {
                sleep_ms: Some(200),
                ..Default::default()
            },
            WorkflowStep {
                verify: Some("First".to_string()),
                ..Default::default()
            },
            WorkflowStep {
                verify: Some("Second".to_string()),
                ..Default::default()
            },
        ];

        let groups = group_steps(&steps);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].step_indices, vec![0, 1, 2, 3, 4]);
        assert_eq!(groups[0].action_indices, vec![0, 1]);
    }
}
