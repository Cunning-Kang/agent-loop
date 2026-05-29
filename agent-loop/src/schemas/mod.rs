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

/// Evidence schema version
pub const EVIDENCE_SCHEMA: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "evidence",
  "type": "object",
  "required": ["schema_version", "task_id", "execution_completeness", "changed_files", "verification_results"],
  "properties": {
    "schema_version": { "type": "string", "enum": ["evidence-v1"] },
    "task_id": {
      "type": "string",
      "pattern": "^task-[0-9]{8}-[0-9]{3}$"
    },
    "execution_completeness": {
      "type": "string",
      "enum": ["full", "partial", "unavailable"]
    },
    "changed_files": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["path", "operation"],
        "properties": {
          "path": { "type": "string" },
          "operation": { "type": "string", "enum": ["create", "modify", "delete"] }
        }
      }
    },
    "verification_results": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["command", "exit_code"],
        "properties": {
          "command": { "type": "string" },
          "exit_code": { "type": "integer" },
          "passed": { "type": "boolean" }
        }
      }
    },
    "external_verdict": {
      "type": ["string", "null"],
      "enum": ["pass", "fail", "blocked", null]
    },
    "execution_trace_available": { "type": "boolean" },
    "audit_available": { "type": "boolean" }
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
