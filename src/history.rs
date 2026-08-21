//! Host-injected command history contracts.

use std::cell::RefCell;
use std::fmt::Debug;
use std::io;
use std::rc::Rc;

/// A host-owned command history used by Rubash's history-facing builtins.
pub trait HistoryProvider: Debug {
    /// Return commands in oldest-to-newest order.
    fn entries(&mut self) -> io::Result<Vec<String>>;
    /// Remove all commands from the host history.
    fn clear(&mut self) -> io::Result<()>;
    /// Append one command to the host history.
    fn append(&mut self, command: String) -> io::Result<()>;
    /// Replace all commands while preserving the host storage implementation.
    fn replace(&mut self, entries: Vec<String>) -> io::Result<()>;
}

/// Shared provider handle suitable for injecting into an Executor.
pub type SharedHistoryProvider = Rc<RefCell<dyn HistoryProvider>>;
