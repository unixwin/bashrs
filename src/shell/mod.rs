//! Shell semantic state owners.

pub mod arrays;
pub mod state;
pub mod variables;

pub use state::ShellState;
pub use variables::{ShellValue, Variable, VariableStore};
