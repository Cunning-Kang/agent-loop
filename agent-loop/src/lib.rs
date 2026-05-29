//! agent-loop library crate.
//!
//! Public API for CLI commands and artifact management.

pub mod artifacts;
pub mod commands;
pub mod id;
pub mod schemas;

pub use artifacts::{AgentRunsPaths, StatusJson, TaskStatus, WorktreePaths};
pub use commands::{CommandError, CommandResult, GateCheck, InitRun, ListRuns, ValidateDiscovery};
pub use id::{validate_id, ContractId, IdKind, PlanId, TaskId};
