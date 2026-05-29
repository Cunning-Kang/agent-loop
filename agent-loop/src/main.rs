//! agent-loop CLI - Phase 1 workflow for Claude Code.
//!
//! Commands:
//! - init-run: Initialize a task run directory with status.json
//! - validate-discovery: Validate codebase discovery JSON schema
//! - list-runs: List actionable runs (active, blocked, cleanup-eligible)
//! - gate-check: Validate task artifacts without semantic judgment

mod artifacts;
mod commands;
mod id;
mod schemas;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Agent-loop CLI for Phase 1 workflow.
#[derive(Parser)]
#[command(
    name = "agent-loop",
    about = "CLI for agent-loop Phase 1 workflow",
    version = "0.1.0"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new task run directory.
    InitRun {
        /// Plan ID (format: plan-YYYYMMDD-NNN)
        #[arg(long)]
        plan_id: String,

        /// Contract ID (format: contract-NNN)
        #[arg(long)]
        contract_id: String,

        /// Task ID (format: task-YYYYMMDD-NNN). If not provided, generates new ID.
        #[arg(long)]
        task_id: Option<String>,

        /// Sequence number for auto-generated task ID (default: 1)
        #[arg(long)]
        sequence: Option<u16>,

        /// Repository root (default: current directory)
        #[arg(long, default_value = ".")]
        repo_root: PathBuf,
    },

    /// Validate a codebase discovery JSON file against the schema.
    ValidateDiscovery {
        /// Path to discovery JSON file
        #[arg(default_value = ".claude/agents/codebase-discovery.json")]
        path: PathBuf,

        /// Suppress output on success
        #[arg(long, short)]
        quiet: bool,
    },

    /// List actionable task runs (active, blocked, cleanup-eligible).
    ListRuns {
        /// Show all runs including abandoned
        #[arg(long, short)]
        all: bool,

        /// Filter by status (active, committed, blocked, abandoned)
        #[arg(long)]
        status: Option<String>,

        /// Filter by plan ID
        #[arg(long)]
        plan_id: Option<String>,

        /// Suppress output on success
        #[arg(long, short)]
        quiet: bool,

        /// Repository root (default: current directory)
        #[arg(long, default_value = ".")]
        repo_root: PathBuf,
    },

    /// Validate task artifacts without semantic judgment.
    GateCheck {
        /// Task ID to check
        #[arg(long)]
        task_id: Option<String>,

        /// Plan ID to check
        #[arg(long)]
        plan_id: Option<String>,

        /// Check contract files
        #[arg(long)]
        check_contracts: bool,

        /// Check status files
        #[arg(long)]
        check_status: bool,

        /// Repository root (default: current directory)
        #[arg(long, default_value = ".")]
        repo_root: PathBuf,
    },
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::InitRun {
            plan_id,
            contract_id,
            task_id,
            sequence,
            repo_root,
        } => commands::InitRun {
            repo_root,
            plan_id,
            contract_id,
            task_id,
            sequence,
        }
        .run(),

        Commands::ValidateDiscovery { path, quiet } => {
            commands::ValidateDiscovery { path, quiet }.run()
        }

        Commands::ListRuns {
            all,
            status,
            plan_id,
            quiet,
            repo_root,
        } => {
            let status_filter = status.and_then(|s| match s.as_str() {
                "active" => Some(artifacts::TaskStatus::Active),
                "committed" => Some(artifacts::TaskStatus::Committed),
                "blocked" => Some(artifacts::TaskStatus::Blocked),
                "abandoned" => Some(artifacts::TaskStatus::Abandoned),
                _ => None,
            });

            commands::ListRuns {
                repo_root,
                all,
                status_filter,
                plan_filter: plan_id,
                quiet,
            }
            .run()
        }

        Commands::GateCheck {
            task_id,
            plan_id,
            check_contracts,
            check_status,
            repo_root,
        } => commands::GateCheck {
            repo_root,
            task_id,
            plan_id,
            check_contracts,
            check_status,
        }
        .run(),
    };

    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::ExitCode::FAILURE
        }
    }
}
