//! Shared shell state boundary.

use super::variables::VariableStore;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ShellOptions {
    pub errexit: bool,
    pub nounset: bool,
    pub pipefail: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PositionalParams {
    values: Vec<String>,
}

impl PositionalParams {
    pub fn new(values: Vec<String>) -> Self {
        Self { values }
    }
    pub fn as_slice(&self) -> &[String] {
        &self.values
    }
    pub fn set(&mut self, values: Vec<String>) {
        self.values = values;
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ShellStatus {
    pub exit_code: i32,
    pub pipestatus: Vec<i32>,
    pub subshell_depth: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ShellState {
    pub variables: VariableStore,
    pub options: ShellOptions,
    pub positional: PositionalParams,
    pub status: ShellStatus,
}

impl ShellState {
    pub fn set_exit_code(&mut self, code: i32) {
        self.status.exit_code = code;
    }
    pub fn set_pipestatus(&mut self, statuses: Vec<i32>) {
        self.status.pipestatus = statuses;
    }
}
