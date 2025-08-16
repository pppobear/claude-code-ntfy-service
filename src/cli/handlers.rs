//! Command handlers for all CLI operations
//! 
//! This module provides clean separation between CLI parsing and business logic
//! by organizing handlers into focused submodules.

mod hook;
mod config;
mod daemon;
mod test;
mod templates;
mod traits;

use super::{Commands, CliContext};
use anyhow::Result;
use traits::HandlerBuilder;

/// Coordinates all command handling operations with dependency injection via CliContext
pub struct CommandHandler {
    context: CliContext,
}

impl CommandHandler {
    /// Create a new command handler instance with the provided context
    pub fn new(context: CliContext) -> Self {
        Self { 
            context,
        }
    }

    /// Route commands to their appropriate handlers
    pub async fn handle_command(&self, command: Commands) -> Result<()> {
        let builder = HandlerBuilder::new(&self.context);
        
        match command {
            Commands::Hook { hook_name, no_daemon, dry_run } => {
                let hook_handler = builder.create_with_context::<hook::HookHandler>();
                hook_handler.handle_hook(hook_name, no_daemon, dry_run).await
            }
            Commands::Init { global, force } => {
                let config_handler = builder.create_with_context::<config::ConfigHandler>();
                config_handler.handle_init(global, force).await
            }
            Commands::Config { action } => {
                let config_handler = builder.create_with_context::<config::ConfigHandler>();
                config_handler.handle_config(action).await
            }
            Commands::Daemon { action } => {
                let daemon_handler = builder.create_with_context::<daemon::DaemonHandler>();
                daemon_handler.handle_daemon(action).await
            }
            Commands::Test { message, title, priority, topic } => {
                let test_handler = builder.create_with_context::<test::TestHandler>();
                test_handler.handle_test(message, title, priority, topic).await
            }
            Commands::Templates { show } => {
                let template_handler = HandlerBuilder::create_stateless::<templates::TemplateHandler>();
                template_handler.handle_templates(show).await
            }
        }
    }
}