//! CLI module providing command-line interface functionality
//! 
//! This module handles argument parsing, command validation, and routing
//! to appropriate handlers while maintaining separation of concerns.

pub mod commands;
pub mod handlers;
pub mod context;

use anyhow::Result;
use clap::Parser;

pub use commands::{Cli, Commands, ConfigAction, DaemonAction};
pub use handlers::CommandHandler;
pub use context::CliContext;

/// Main CLI application following the new CliContext pattern
pub struct CliApp;

impl CliApp {
    /// Parse command line arguments and execute the requested command
    pub async fn run() -> Result<()> {
        let cli = Cli::parse();

        // Create CLI context with project path and verbosity
        let context = CliContext::new(cli.project.clone(), cli.verbose)?;
        
        // Initialize logging through context
        context.init_logging()?;

        // Create command handler with context
        let handler = CommandHandler::new(context);

        // Handle default hook mode when called without subcommand
        let command = cli.command.unwrap_or_else(|| Commands::Hook {
            hook_name: None,
            no_daemon: false,
            dry_run: false,
        });

        // Execute the command through handlers
        handler.handle_command(command).await
    }
}