//! agent-loop library crate.
//!
//! Public API for CLI commands and artifact management.

pub mod artifacts;
pub mod backend;
pub mod commands;
pub mod evidence;
pub mod id;
pub mod schemas;

pub use backend::{Backend, BackendOutcome, MockBackend, VerificationEntry};

pub use artifacts::{AgentRunsPaths, StatusJson, TaskStatus, WorktreePaths, find_repo_root, read_json};
pub use commands::{
    CollectEvidence, CommandError, CommandResult, FinalGate, FinalGateDecision, GateCheck,
    GitGuard, InitRun, Integrate, ListRuns, OpusFinalGate, ValidateDiscovery, ValidateEvidence,
    ValidateSonnetReview, ValidateSubagentStop, PostApplyVerification, IntegrationResult,
    CommandRunResult, PreToolCheck,
};
pub use evidence::{
    collect_evidence, validate_evidence, ChangedFileEntry, ChangedFilesDoc,
    CollectEvidenceInputs, ExternalReviewDoc, ExternalReviewFinding, ExecutionTraceEvent,
    FinalEvidenceArtifacts, FinalEvidenceDoc, REQUIRED_ARTIFACTS, SensitiveAuditDoc,
    ValidationReport, VerificationDoc, VerificationResult, WorktreeHandle,
};
pub use id::{validate_id, ContractId, IdKind, PlanId, TaskId};
