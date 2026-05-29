//! JSON Schema definitions for agent-loop artifacts.
//!
//! All snake_case field naming enforced per ADR-001.
//! Schemas are embedded as static strings and used for validation.

/// Discovery schema version
pub const CODEBASE_DISCOVERY_SCHEMA: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "codebase_discovery",
  "type": "object",
  "required": ["schema_version", "repo_facts", "available_scripts", "relevant_files", "relevant_tests", "verification_candidates", "scope_candidates", "risk_hints", "unknowns", "discovery_limits"],
  "properties": {
    "schema_version": { "type": "string", "enum": ["codebase-discovery-v1"] },
    "repo_facts": {
      "type": "object",
      "properties": {
        "root_path": { "type": "string" },
        "languages": { "type": "array", "items": { "type": "string" } },
        "package_managers": { "type": "array", "items": { "type": "string" } },
        "frameworks": { "type": "array", "items": { "type": "string" } },
        "has_tests": { "type": "boolean" },
        "ci_system": { "type": ["string", "null"] }
      },
      "required": ["root_path", "languages", "package_managers", "frameworks", "has_tests"],
      "additionalProperties": true
    },
    "available_scripts": {
      "type": "object",
      "additionalProperties": { "type": "string" }
    },
    "relevant_files": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["path", "description"],
        "properties": {
          "path": { "type": "string" },
          "description": { "type": "string" },
          "test_file": { "type": "boolean" }
        }
      }
    },
    "relevant_tests": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["path", "test_type", "covers"],
        "properties": {
          "path": { "type": "string" },
          "test_type": { "type": "string" },
          "covers": { "type": "array", "items": { "type": "string" } }
        }
      }
    },
    "verification_candidates": {
      "type": "array",
      "items": { "type": "string" }
    },
    "scope_candidates": {
      "type": "array",
      "items": { "type": "string" }
    },
    "risk_hints": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["category", "description"],
        "properties": {
          "category": { "type": "string" },
          "description": { "type": "string" },
          "severity": { "type": "string", "enum": ["low", "medium", "high"] }
        }
      }
    },
    "unknowns": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["description"],
        "properties": {
          "description": { "type": "string" },
          "impact": { "type": "string" }
        }
      }
    },
    "discovery_limits": {
      "type": "array",
      "items": { "type": "string" }
    }
  },
  "additionalProperties": true
}"#;

/// Plan schema version
pub const PLAN_SCHEMA: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "plan",
  "type": "object",
  "required": ["schema_version", "plan_id", "created_at", "contracts"],
  "properties": {
    "schema_version": { "type": "string", "enum": ["plan-v1"] },
    "plan_id": {
      "type": "string",
      "pattern": "^plan-[0-9]{8}-[0-9]{3}$"
    },
    "created_at": { "type": "string", "format": "date-time" },
    "updated_at": { "type": "string", "format": "date-time" },
    "objective": { "type": "string" },
    "contracts": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["contract_id", "status"],
        "properties": {
          "contract_id": {
            "type": "string",
            "pattern": "^contract-[0-9]{3}$"
          },
          "status": {
            "type": "string",
            "enum": ["proposed", "approved", "superseded", "rejected"]
          },
          "task_id": { "type": ["string", "null"] }
        }
      }
    },
    "discovery_ref": {
      "type": ["object", "null"],
      "properties": {
        "path": { "type": "string" }
      }
    }
  },
  "additionalProperties": true
}"#;

/// Contract schema version
pub const CONTRACT_SCHEMA: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "task_contract",
  "type": "object",
  "required": ["schema_version", "plan_id", "contract_id", "status", "objective", "risk_class", "execution_eligibility", "scope", "acceptance_criteria", "required_verification", "optional_verification", "mutation_policy", "test_policy", "repair_budget", "discovery_usage"],
  "properties": {
    "schema_version": { "type": "string", "enum": ["task-contract-v1"] },
    "plan_id": {
      "type": "string",
      "pattern": "^plan-[0-9]{8}-[0-9]{3}$"
    },
    "contract_id": {
      "type": "string",
      "pattern": "^contract-[0-9]{3}$"
    },
    "task_id": { "type": ["string", "null"] },
    "status": {
      "type": "string",
      "enum": ["proposed", "approved", "superseded", "rejected"]
    },
    "objective": { "type": "string" },
    "non_goals": {
      "type": "array",
      "items": { "type": "string" }
    },
    "risk_class": {
      "type": "string",
      "enum": ["low", "normal", "high"]
    },
    "risk_basis": { "type": "string" },
    "execution_eligibility": {
      "type": "object",
      "required": ["allowed"],
      "properties": {
        "allowed": { "type": "boolean" },
        "blocked_reason": { "type": ["string", "null"] },
        "details": { "type": ["string", "null"] }
      }
    },
    "scope": {
      "type": "array",
      "items": { "type": "string" }
    },
    "acceptance_criteria": {
      "type": "array",
      "items": { "type": "string" }
    },
    "required_verification": {
      "type": "array",
      "items": { "type": "string" }
    },
    "optional_verification": {
      "type": "array",
      "items": { "type": "string" }
    },
    "mutation_policy": {
      "type": "object",
      "properties": {
        "allowed_patterns": { "type": "array", "items": { "type": "string" } },
        "forbidden_patterns": { "type": "array", "items": { "type": "string" } }
      }
    },
    "test_policy": {
      "type": "object",
      "properties": {
        "allowed": { "type": "array", "items": { "type": "string" } },
        "forbidden": { "type": "array", "items": { "type": "string" } }
      }
    },
    "repair_budget": { "type": "integer", "minimum": 0, "maximum": 3 },
    "discovery_usage": {
      "oneOf": [
        {
          "type": "object",
          "required": ["used"],
          "properties": {
            "used": { "const": true },
            "source_path": { "type": "string" },
            "validation_status": { "type": "string" },
            "adopted": { "type": "array", "items": { "type": "string" } },
            "modified": { "type": "array", "items": { "type": "string" } },
            "rejected": { "type": "array", "items": { "type": "string" } },
            "unresolved_unknowns": { "type": "array", "items": { "type": "string" } }
          }
        },
        {
          "type": "object",
          "required": ["used"],
          "properties": {
            "used": { "const": false },
            "reason": { "type": "string" }
          }
        }
      ]
    },
    "approval": {
      "type": ["object", "null"],
      "properties": {
        "approved_by": { "type": "string" },
        "approved_at": { "type": "string", "format": "date-time" },
        "notes": { "type": ["string", "null"] }
      }
    }
  }
}"#;

/// Status schema version
pub const STATUS_SCHEMA: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "task_status",
  "type": "object",
  "required": ["schema_version", "task_id", "plan_id", "contract_id", "status", "updated_at"],
  "properties": {
    "schema_version": { "type": "string", "enum": ["status-v1"] },
    "task_id": {
      "type": "string",
      "pattern": "^task-[0-9]{8}-[0-9]{3}$"
    },
    "plan_id": {
      "type": "string",
      "pattern": "^plan-[0-9]{8}-[0-9]{3}$"
    },
    "contract_id": {
      "type": "string",
      "pattern": "^contract-[0-9]{3}$"
    },
    "status": {
      "type": "string",
      "enum": ["active", "committed", "blocked", "abandoned"]
    },
    "blocked_reason": {
      "type": ["string", "null"]
    },
    "details": {
      "type": ["string", "null"]
    },
    "created_at": { "type": "string", "format": "date-time" },
    "updated_at": { "type": "string", "format": "date-time" }
  },
  "if": {
    "properties": { "status": { "const": "blocked" } },
    "required": ["status"]
  },
  "then": {
    "required": ["blocked_reason", "details"]
  }
}"#;


/// Machine gate schema version
pub const MACHINE_GATE_SCHEMA: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "machine_gate",
  "type": "object",
  "required": ["schema_version", "task_id", "passed", "checks"],
  "properties": {
    "schema_version": { "type": "string", "enum": ["machine-gate-v1"] },
    "task_id": {
      "type": "string",
      "pattern": "^task-[0-9]{8}-[0-9]{3}$"
    },
    "passed": { "type": "boolean" },
    "checks": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["name", "passed"],
        "properties": {
          "name": { "type": "string" },
          "passed": { "type": "boolean" },
          "details": { "type": ["string", "null"] }
        }
      }
    }
  }
}"#;
/// Changed-files schema version
pub const CHANGED_FILES_SCHEMA: &str = r#"{
 "$schema": "http://json-schema.org/draft-07/schema#",
 "title": "changed_files",
 "type": "object",
 "required": ["schema_version", "task_id", "files"],
 "properties": {
 "schema_version": { "type": "string", "enum": ["changed-files-v1"] },
 "task_id": { "type": "string", "pattern": "^task-[0-9]{8}-[0-9]{3}$" },
 "files": {
 "type": "array",
 "items": {
 "type": "object",
 "required": ["path", "operation"],
 "properties": {
 "path": { "type": "string", "minLength":1 },
 "operation": { "type": "string", "enum": ["create", "modify", "delete"] }
 },
 "additionalProperties": true
 }
 }
 },
 "additionalProperties": true
}"#;

/// Execution-trace schema version (each JSONL line).
/// `execution_trace.jsonl` is a sequence of such lines joined by '\n'.
pub const EXECUTION_TRACE_LINE_SCHEMA: &str = r#"{
 "$schema": "http://json-schema.org/draft-07/schema#",
 "title": "execution_trace_line",
 "type": "object",
 "required": ["schema_version", "task_id", "event", "timestamp"],
 "properties": {
 "schema_version": { "type": "string", "enum": ["execution-trace-v1"] },
 "task_id": { "type": "string", "pattern": "^task-[0-9]{8}-[0-9]{3}$" },
 "event": { "type": "string", "minLength":1 },
 "timestamp": { "type": "string", "format": "date-time" },
 "details": { "type": "object" }
 },
 "additionalProperties": true
}"#;

/// Verification schema version
pub const VERIFICATION_SCHEMA: &str = r#"{
 "$schema": "http://json-schema.org/draft-07/schema#",
 "title": "verification",
 "type": "object",
 "required": ["schema_version", "task_id", "results"],
 "properties": {
 "schema_version": { "type": "string", "enum": ["verification-v1"] },
 "task_id": { "type": "string", "pattern": "^task-[0-9]{8}-[0-9]{3}$" },
 "results": {
 "type": "array",
 "items": {
 "type": "object",
 "required": ["command", "exit_code", "passed"],
 "properties": {
 "command": { "type": "string", "minLength":1 },
 "exit_code": { "type": "integer" },
 "passed": { "type": "boolean" },
 "stdout_excerpt": { "type": "string" },
 "stderr_excerpt": { "type": "string" }
 },
 "additionalProperties": true
 }
 },
 "all_required_passed": { "type": "boolean" }
 },
 "additionalProperties": true
}"#;

/// External review schema version
pub const EXTERNAL_REVIEW_SCHEMA: &str = r#"{
 "$schema": "http://json-schema.org/draft-07/schema#",
 "title": "external_review",
 "type": "object",
 "required": ["schema_version", "task_id", "verdict"],
 "properties": {
 "schema_version": { "type": "string", "enum": ["external-review-v1"] },
 "task_id": { "type": "string", "pattern": "^task-[0-9]{8}-[0-9]{3}$" },
 "verdict": { "type": "string", "enum": ["pass", "fail", "blocked"] },
 "scope_compliance": { "type": "boolean" },
 "policy_compliance": { "type": "boolean" },
 "verification_sufficient": { "type": "boolean" },
 "summary": { "type": "string" },
 "findings": {
 "type": "array",
 "items": {
 "type": "object",
 "required": ["category", "severity", "description"],
 "properties": {
 "category": { "type": "string" },
 "severity": { "type": "string", "enum": ["low", "medium", "high", "info"] },
 "description": { "type": "string" }
 },
 "additionalProperties": true
 }
 }
 },
 "additionalProperties": true
}"#;

/// Sensitive audit schema version
pub const SENSITIVE_AUDIT_SCHEMA: &str = r#"{
 "$schema": "http://json-schema.org/draft-07/schema#",
 "title": "sensitive_audit",
 "type": "object",
 "required": ["schema_version", "task_id", "detector", "blocked"],
 "properties": {
 "schema_version": { "type": "string", "enum": ["sensitive-audit-v1"] },
 "task_id": { "type": "string", "pattern": "^task-[0-9]{8}-[0-9]{3}$" },
 "detector": { "type": "string", "minLength":1 },
 "blocked": { "type": "boolean" }
 },
 "sensitive_paths_touched": {
 "type": "array",
 "items": { "type": "string" }
 },
 "forbidden_patterns_violated": {
 "type": "array",
 "items": { "type": "string" }
 },
 "limitations": {
 "type": "array",
 "items": { "type": "string" }
 },
 "additionalProperties": true
}"#;

/// Final evidence schema version
pub const FINAL_EVIDENCE_SCHEMA: &str = r#"{
 "$schema": "http://json-schema.org/draft-07/schema#",
 "title": "final_evidence",
 "type": "object",
 "required": ["schema_version", "task_id", "execution_completeness", "external_verdict", "artifacts"],
 "properties": {
 "schema_version": { "type": "string", "enum": ["final-evidence-v1"] },
 "task_id": {
 "type": "string",
 "pattern": "^task-[0-9]{8}-[0-9]{3}$"
 },
 "execution_completeness": { "type": "string", "enum": ["full", "partial", "unavailable"] },
 "external_verdict": { "type": ["string", "null"], "enum": ["pass", "fail", "blocked", null] },
 "artifacts": {
 "type": "object",
 "required": [
 "changed_files",
 "diff_patch",
 "execution_trace",
 "verification",
 "external_review",
 "sensitive_audit",
 "final_evidence"
 ],
 "properties": {
 "changed_files": { "type": "string" },
 "diff_patch": { "type": "string" },
 "execution_trace": { "type": "string" },
 "verification": { "type": "string" },
 "external_review": { "type": "string" },
 "sensitive_audit": { "type": "string" },
 "final_evidence": { "type": "string" }
 },
 "additionalProperties": true
 },
 "audit_limitations": {
 "type": "array",
 "items": { "type": "string" }
 }
 },
 "additionalProperties": true
}"#;
