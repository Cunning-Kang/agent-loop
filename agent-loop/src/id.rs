//! ID generation and validation for plan, contract, and task identifiers.
//!
//! Formats (per ADR-001):
//! - `plan-YYYYMMDD-NNN`  e.g. plan-20260526-001
//! - `contract-NNN`       e.g. contract-001
//! - `task-YYYYMMDD-NNN`  e.g. task-20260526-001
//!
//! All IDs are zero-padded to 3 digits for sequence numbers.

use chrono::{Datelike, Local, NaiveDate};
use regex::Regex;
use std::sync::LazyLock;

/// Regex for plan_id: plan-YYYYMMDD-NNN
static PLAN_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^plan-\d{8}-\d{3}$").unwrap());

/// Regex for contract_id: contract-NNN
static CONTRACT_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^contract-\d{3}$").unwrap());

/// Regex for task_id: task-YYYYMMDD-NNN
static TASK_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^task-\d{8}-\d{3}$").unwrap());

/// Plan ID: plan-YYYYMMDD-NNN
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanId(String);

impl PlanId {
    /// Generate a new plan ID with today's date and next sequence.
    pub fn generate(sequence: u16) -> Self {
        let today = Local::now();
        let date_str = format!(
            "{:04}{:02}{:02}",
            today.year(),
            today.month() as u8,
            today.day()
        );
        Self(format!("plan-{}-{:03}", date_str, sequence))
    }

    /// Parse from string, validating format.
    pub fn parse(s: &str) -> Option<Self> {
        if PLAN_ID_RE.is_match(s) {
            Some(Self(s.to_string()))
        } else {
            None
        }
    }

    /// Extract date portion (YYYYMMDD) from plan ID.
    pub fn date(&self) -> Option<NaiveDate> {
        let date_str = self.0.strip_prefix("plan-")?.split('-').next()?;
        NaiveDate::parse_from_str(date_str, "%Y%m%d").ok()
    }

    /// Extract sequence number from plan ID.
    pub fn sequence(&self) -> Option<u16> {
        let parts: Vec<&str> = self.0.split('-').collect();
        parts.get(2)?.parse().ok()
    }
}

impl std::fmt::Display for PlanId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for PlanId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Contract ID: contract-NNN
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractId(String);

impl ContractId {
    /// Generate a new contract ID with given sequence (1-based).
    pub fn generate(sequence: u16) -> Self {
        Self(format!("contract-{:03}", sequence))
    }

    /// Parse from string, validating format.
    pub fn parse(s: &str) -> Option<Self> {
        if CONTRACT_ID_RE.is_match(s) {
            Some(Self(s.to_string()))
        } else {
            None
        }
    }

    /// Extract sequence number from contract ID.
    pub fn sequence(&self) -> Option<u16> {
        let num_str = self.0.strip_prefix("contract-")?;
        num_str.parse().ok()
    }
}

impl std::fmt::Display for ContractId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for ContractId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Task ID: task-YYYYMMDD-NNN
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskId(String);

impl TaskId {
    /// Generate a new task ID with today's date and next sequence.
    pub fn generate(sequence: u16) -> Self {
        let today = Local::now();
        let date_str = format!(
            "{:04}{:02}{:02}",
            today.year(),
            today.month() as u8,
            today.day()
        );
        Self(format!("task-{}-{:03}", date_str, sequence))
    }

    /// Parse from string, validating format.
    pub fn parse(s: &str) -> Option<Self> {
        if TASK_ID_RE.is_match(s) {
            Some(Self(s.to_string()))
        } else {
            None
        }
    }

    /// Extract date portion from task ID.
    pub fn date(&self) -> Option<NaiveDate> {
        let date_str = self.0.strip_prefix("task-")?.split('-').next()?;
        NaiveDate::parse_from_str(date_str, "%Y%m%d").ok()
    }

    /// Extract sequence number from task ID.
    pub fn sequence(&self) -> Option<u16> {
        let parts: Vec<&str> = self.0.split('-').collect();
        parts.get(2)?.parse().ok()
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for TaskId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Validate any ID string against expected format.
pub fn validate_id(id: &str, kind: IdKind) -> bool {
    match kind {
        IdKind::Plan => PLAN_ID_RE.is_match(id),
        IdKind::Contract => CONTRACT_ID_RE.is_match(id),
        IdKind::Task => TASK_ID_RE.is_match(id),
    }
}

/// Get the regex pattern string for an ID kind (for documentation/errors).
pub fn id_pattern(kind: IdKind) -> &'static str {
    match kind {
        IdKind::Plan => r"^plan-YYYYMMDD-NNN$",
        IdKind::Contract => r"^contract-NNN$",
        IdKind::Task => r"^task-YYYYMMDD-NNN$",
    }
}

#[derive(Debug, Clone, Copy)]
pub enum IdKind {
    Plan,
    Contract,
    Task,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_id_generation() {
        let id = PlanId::generate(1);
        assert!(PLAN_ID_RE.is_match(id.as_ref()));
        assert!(id.as_ref().starts_with("plan-"));

        let id5 = PlanId::generate(5);
        assert!(id5.as_ref().ends_with("-005"));

        let id999 = PlanId::generate(999);
        assert!(id999.as_ref().ends_with("-999"));
    }

    #[test]
    fn test_plan_id_parse() {
        let valid = PlanId::parse("plan-20260526-001");
        assert!(valid.is_some());
        assert_eq!(valid.unwrap().as_ref(), "plan-20260526-001");

        let invalid = PlanId::parse("plan-20260526-1");
        assert!(invalid.is_none());

        let invalid2 = PlanId::parse("task-20260526-001");
        assert!(invalid2.is_none());
    }

    #[test]
    fn test_plan_id_date_extraction() {
        let id = PlanId::parse("plan-20260526-001").unwrap();
        let date = id.date().unwrap();
        assert_eq!(date, NaiveDate::from_ymd_opt(2026, 5, 26).unwrap());
    }

    #[test]
    fn test_plan_id_sequence_extraction() {
        let id = PlanId::parse("plan-20260526-001").unwrap();
        assert_eq!(id.sequence(), Some(1));

        let id99 = PlanId::parse("plan-20260526-099").unwrap();
        assert_eq!(id99.sequence(), Some(99));
    }

    #[test]
    fn test_contract_id_generation() {
        let id = ContractId::generate(1);
        assert!(CONTRACT_ID_RE.is_match(id.as_ref()));
        assert_eq!(id.as_ref(), "contract-001");

        let id5 = ContractId::generate(5);
        assert_eq!(id5.as_ref(), "contract-005");

        let id99 = ContractId::generate(99);
        assert_eq!(id99.as_ref(), "contract-099");
    }

    #[test]
    fn test_contract_id_parse() {
        let valid = ContractId::parse("contract-001");
        assert!(valid.is_some());
        assert_eq!(valid.unwrap().as_ref(), "contract-001");

        let invalid = ContractId::parse("contract-1");
        assert!(invalid.is_none());

        let invalid2 = ContractId::parse("contract-0001");
        assert!(invalid2.is_none());

        let invalid3 = ContractId::parse("plan-20260526-001");
        assert!(invalid3.is_none());
    }

    #[test]
    fn test_task_id_generation() {
        let id = TaskId::generate(1);
        assert!(TASK_ID_RE.is_match(id.as_ref()));
        assert!(id.as_ref().starts_with("task-"));
    }

    #[test]
    fn test_task_id_parse() {
        let valid = TaskId::parse("task-20260526-001");
        assert!(valid.is_some());

        let invalid = TaskId::parse("task-20260526-1");
        assert!(invalid.is_none());

        let invalid2 = TaskId::parse("task-2026-0526-001");
        assert!(invalid2.is_none());
    }

    #[test]
    fn test_task_id_date_extraction() {
        let id = TaskId::parse("task-20260526-001").unwrap();
        let date = id.date().unwrap();
        assert_eq!(date, NaiveDate::from_ymd_opt(2026, 5, 26).unwrap());
    }

    #[test]
    fn test_validate_id() {
        assert!(validate_id("plan-20260526-001", IdKind::Plan));
        assert!(!validate_id("plan-20260526-1", IdKind::Plan));

        assert!(validate_id("contract-001", IdKind::Contract));
        assert!(!validate_id("contract-1", IdKind::Contract));

        assert!(validate_id("task-20260526-001", IdKind::Task));
        assert!(!validate_id("task-20260526-1", IdKind::Task));
    }
}
