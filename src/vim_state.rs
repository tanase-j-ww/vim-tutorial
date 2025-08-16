use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VimState {
    pub mode: VimMode,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub operator: Option<String>,
    pub buffer_content: Vec<String>,
    pub registers: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VimMode {
    Normal,
    Insert,
    Visual,
    VisualLine,
    VisualBlock,
    OperatorPending(String), // オペレーター待機モード（オペレーター名付き）
    Command,
}

impl VimMode {
    pub fn from_vim_mode(mode: &str, mode_detailed: &str, operator: Option<String>) -> Self {
        match (mode, mode_detailed) {
            ("n", "no") => VimMode::OperatorPending(operator.unwrap_or_default()),
            ("n", _) => VimMode::Normal,
            ("i", _) => VimMode::Insert,
            ("v", _) => VimMode::Visual,
            ("V", _) => VimMode::VisualLine,
            (mode_str, _) if mode_str.contains('\u{16}') => VimMode::VisualBlock, // Ctrl-V
            ("c", _) => VimMode::Command,
            _ => VimMode::Normal,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GoalType {
    Position { line: usize, col: usize },
    Mode(VimMode),
    TextContent { line: usize, expected: String },
    BufferChange,
    RegisterContent { register: String, expected: String },
}

#[derive(Debug, Clone)]
pub struct Goal {
    pub goal_type: GoalType,
    #[allow(dead_code)] // 将来の機能拡張で使用予定
    pub description: String,
}

pub struct GoalDetector;

impl GoalDetector {
    pub fn new() -> Self {
        Self
    }

    pub fn check_goal(&self, goal: &Goal, current_state: &VimState) -> bool {
        match &goal.goal_type {
            GoalType::Position { line, col } => {
                current_state.cursor_line == *line && current_state.cursor_col == *col
            }
            GoalType::Mode(expected_mode) => &current_state.mode == expected_mode,
            GoalType::TextContent { line, expected } => {
                if let Some(actual_line) = current_state.buffer_content.get(*line) {
                    actual_line == expected
                } else {
                    false
                }
            }
            GoalType::BufferChange => {
                // バッファが変更されているかは前の状態と比較する必要があるため、
                // この実装では単純化してtrueを返す
                // 実際の実装では前の状態との比較が必要
                true
            }
            GoalType::RegisterContent { register, expected } => {
                if let Some(actual_content) = current_state.registers.get(register) {
                    actual_content == expected
                } else {
                    false
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_state() -> VimState {
        VimState {
            mode: VimMode::Normal,
            cursor_line: 1,
            cursor_col: 1,
            operator: None,
            buffer_content: vec!["hello world".to_string(), "second line".to_string()],
            registers: HashMap::new(),
        }
    }

    #[test]
    fn test_vim_mode_from_vim_mode() {
        assert_eq!(VimMode::from_vim_mode("n", "n", None), VimMode::Normal);
        assert_eq!(
            VimMode::from_vim_mode("n", "no", Some("d".to_string())),
            VimMode::OperatorPending("d".to_string())
        );
        assert_eq!(VimMode::from_vim_mode("i", "i", None), VimMode::Insert);
        assert_eq!(VimMode::from_vim_mode("v", "v", None), VimMode::Visual);
        assert_eq!(VimMode::from_vim_mode("V", "V", None), VimMode::VisualLine);
    }

    #[test]
    fn test_position_goal_detection() {
        let detector = GoalDetector::new();
        let mut state = create_test_state();

        let goal = Goal {
            goal_type: GoalType::Position { line: 1, col: 1 },
            description: "Move to position 1,1".to_string(),
        };

        assert!(detector.check_goal(&goal, &state));

        state.cursor_line = 2;
        assert!(!detector.check_goal(&goal, &state));
    }

    #[test]
    fn test_mode_goal_detection() {
        let detector = GoalDetector::new();
        let mut state = create_test_state();

        let insert_goal = Goal {
            goal_type: GoalType::Mode(VimMode::Insert),
            description: "Enter insert mode".to_string(),
        };

        assert!(!detector.check_goal(&insert_goal, &state));

        state.mode = VimMode::Insert;
        assert!(detector.check_goal(&insert_goal, &state));
    }

    #[test]
    fn test_operator_pending_goal_detection() {
        let detector = GoalDetector::new();
        let mut state = create_test_state();

        let delete_op_goal = Goal {
            goal_type: GoalType::Mode(VimMode::OperatorPending("d".to_string())),
            description: "Press 'd' for delete operation".to_string(),
        };

        assert!(!detector.check_goal(&delete_op_goal, &state));

        state.mode = VimMode::OperatorPending("d".to_string());
        assert!(detector.check_goal(&delete_op_goal, &state));
    }

    #[test]
    fn test_text_content_goal_detection() {
        let detector = GoalDetector::new();
        let state = create_test_state();

        let text_goal = Goal {
            goal_type: GoalType::TextContent {
                line: 0,
                expected: "hello world".to_string(),
            },
            description: "Check first line content".to_string(),
        };

        assert!(detector.check_goal(&text_goal, &state));

        let wrong_text_goal = Goal {
            goal_type: GoalType::TextContent {
                line: 0,
                expected: "different text".to_string(),
            },
            description: "Check wrong content".to_string(),
        };

        assert!(!detector.check_goal(&wrong_text_goal, &state));
    }

    #[test]
    fn test_register_content_goal_detection() {
        let detector = GoalDetector::new();
        let mut state = create_test_state();

        state
            .registers
            .insert("0".to_string(), "yanked_text".to_string());

        let register_goal = Goal {
            goal_type: GoalType::RegisterContent {
                register: "0".to_string(),
                expected: "yanked_text".to_string(),
            },
            description: "Check yank register content".to_string(),
        };

        assert!(detector.check_goal(&register_goal, &state));

        let wrong_register_goal = Goal {
            goal_type: GoalType::RegisterContent {
                register: "1".to_string(),
                expected: "yanked_text".to_string(),
            },
            description: "Check non-existent register".to_string(),
        };

        assert!(!detector.check_goal(&wrong_register_goal, &state));
    }
}
