//! Command definitions and structures for the CLI
//! 
//! This module contains all the clap-based command line argument definitions,
//! including the main CLI structure and all subcommands.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Main CLI structure
#[derive(Parser)]
#[command(name = "claude-ntfy")]
#[command(about = "Claude Code hook CLI tool with ntfy integration")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Project path for project-level configuration
    #[arg(long, global = true)]
    pub project: Option<PathBuf>,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

/// Available CLI commands
#[derive(Subcommand)]
pub enum Commands {
    /// Handle Claude Code hook (default mode when no subcommand)
    Hook {
        /// Hook name (from CLAUDE_HOOK environment variable)
        #[arg(short = 'n', long, env = "CLAUDE_HOOK")]
        hook_name: Option<String>,

        /// Don't send to daemon, process directly
        #[arg(long)]
        no_daemon: bool,

        /// Dry run - don't actually send notification
        #[arg(long)]
        dry_run: bool,
    },

    /// Initialize configuration
    Init {
        /// Initialize global configuration (default is project-level)
        #[arg(short, long)]
        global: bool,

        /// Force overwrite existing configuration
        #[arg(short, long)]
        force: bool,
    },

    /// Configure settings
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Daemon management
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },

    /// Test notification sending
    Test {
        /// Message to send
        message: String,

        /// Optional title
        #[arg(short, long)]
        title: Option<String>,

        /// Priority (1-5)
        #[arg(short, long, default_value = "3")]
        priority: u8,

        /// Topic to send to
        #[arg(short = 'o', long)]
        topic: Option<String>,
    },

    /// List available templates
    Templates {
        /// Show template content
        #[arg(short, long)]
        show: Option<String>,
    },
}

/// Configuration management actions
#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show current configuration
    Show,

    /// Set configuration value
    Set {
        /// Configuration key (e.g., ntfy.server_url)
        key: String,
        /// Value to set
        value: String,
    },

    /// Get configuration value
    Get {
        /// Configuration key
        key: String,
    },

    /// Set hook-specific configuration
    Hook {
        /// Hook name
        name: String,
        /// Topic for this hook
        #[arg(long)]
        topic: Option<String>,
        /// Priority for this hook
        #[arg(long)]
        priority: Option<u8>,
        /// Add filter for this hook
        #[arg(long)]
        filter: Option<String>,
    },
}

/// Daemon management actions
#[derive(Subcommand)]
pub enum DaemonAction {
    /// Start the daemon
    Start {
        /// Run in detached mode (background)
        #[arg(short = 'd', long)]
        detach: bool,
    },

    /// Stop the daemon
    Stop,

    /// Check daemon status
    Status,

    /// Reload daemon configuration
    Reload,
}

impl Commands {
    /// Check if this command requires daemon functionality
    pub fn requires_daemon(&self) -> bool {
        matches!(self, Commands::Daemon { .. })
    }

    /// Check if this command modifies configuration
    pub fn modifies_config(&self) -> bool {
        matches!(
            self,
            Commands::Init { .. } | Commands::Config { action: ConfigAction::Set { .. } | ConfigAction::Hook { .. } }
        )
    }

    /// Get the command name as a string for logging/debugging
    pub fn name(&self) -> &'static str {
        match self {
            Commands::Hook { .. } => "hook",
            Commands::Init { .. } => "init", 
            Commands::Config { .. } => "config",
            Commands::Daemon { .. } => "daemon",
            Commands::Test { .. } => "test",
            Commands::Templates { .. } => "templates",
        }
    }
}

impl ConfigAction {
    /// Check if this action modifies configuration
    pub fn is_mutating(&self) -> bool {
        matches!(self, ConfigAction::Set { .. } | ConfigAction::Hook { .. })
    }
}

impl DaemonAction {
    /// Check if this action requires an active daemon
    pub fn requires_active_daemon(&self) -> bool {
        matches!(self, DaemonAction::Stop | DaemonAction::Status | DaemonAction::Reload)
    }
}