//! agent-loop library crate.
//!
//! Public API for CLI commands and artifact management.

pub mod artifacts;
pub mod commands;
pub mod id;
pub mod schemas;
pub mod evidence;
pub mod backend;
pub use backend::{Backend, BackendOutcome, MockBackend, VerificationEntry};

pub use artifacts::{AgentRunsPaths, StatusJson, TaskStatus, WorktreePaths};
pub use evidence::{collect_evidence, validate_evidence, ChangedFileEntry, ChangedFilesDoc, CollectEvidenceInputs, ExternalReviewDoc, ExternalReviewFinding, ExecutionTraceEvent, FinalEvidenceArtifacts, FinalEvidenceDoc, REQUIRED_ARTIFACTS, SensitiveAuditDoc, ValidationReport, VerificationDoc, VerificationResult, WorktreeHandle};
pub use commands::{CollectEvidence, CommandError, CommandResult, GateCheck, InitRun, ListRuns, ValidateDiscovery, ValidateEvidence, ValidateSonnetReview};
pub use id::{validate_id, ContractId, IdKind, PlanId, TaskId};
