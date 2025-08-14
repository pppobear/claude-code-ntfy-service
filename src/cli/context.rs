//! CLI Context for dependency injection and shared state
//! 
//! This module provides the CliContext abstraction that centralizes
//! configuration management and reduces coupling in CLI handlers.

use std::path::PathBuf;
use std::sync::Arc;
use anyhow::Result;
use crate::config::ConfigManager;

/// CLI execution context containing shared dependencies and configuration
pub struct CliContext {
    pub project_path: Option<PathBuf>,
    pub verbose: bool,
    pub config_manager: Arc<ConfigManager>,
}

impl CliContext {
    /// Create a new CLI context with the specified project path and verbosity
    pub fn new(project_path: Option<PathBuf>, verbose: bool) -> Result<Self> {
        // Auto-detect project path if not specified
        let resolved_project_path = Self::resolve_project_path(project_path);
        let config_manager = Arc::new(ConfigManager::new(resolved_project_path.clone())?);
        
        Ok(Self {
            project_path: resolved_project_path,
            verbose,
            config_manager,
        })
    }
    
    /// Auto-detect project path by looking for .claude/ntfy-service/config.toml
    fn resolve_project_path(project_path: Option<PathBuf>) -> Option<PathBuf> {
        if let Some(path) = project_path {
            return Some(path);
        }
        
        // Check if current directory has .claude/ntfy-service/config.toml
        if let Ok(current_dir) = std::env::current_dir() {
            let config_path = current_dir.join(".claude").join("ntfy-service").join("config.toml");
            if config_path.exists() {
                return Some(current_dir);
            }
        }
        
        // No project config found, use global config
        None
    }

    /// Initialize logging subsystem based on verbosity and configuration
    pub fn init_logging(&self) -> Result<()> {
        let log_level = if self.verbose { 
            "debug" 
        } else { 
            &self.config_manager.config().daemon.log_level
        };
        
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive(log_level.parse().unwrap_or_else(|_| {
                        tracing::Level::INFO.into()
                    })),
            )
            .init();

        if self.verbose {
            tracing::debug!("Verbose logging enabled");
            tracing::debug!("Project path: {:?}", self.project_path);
            tracing::debug!("Config path: {:?}", self.config_manager.config());
        }

        Ok(())
    }

    /// Get a reference to the configuration manager
    pub fn config_manager(&self) -> &Arc<ConfigManager> {
        &self.config_manager
    }

    /// Get the project path if set
    pub fn project_path(&self) -> Option<&PathBuf> {
        self.project_path.as_ref()
    }

    /// Check if verbose mode is enabled
    pub fn is_verbose(&self) -> bool {
        self.verbose
    }

    /// Create a new CliContext with updated project path
    pub fn with_project_path(&self, project_path: Option<PathBuf>) -> Result<Self> {
        Self::new(project_path, self.verbose)
    }

    /// Validate that the context is properly initialized
    pub fn validate(&self) -> Result<()> {
        // Validate configuration manager is accessible
        let _config = self.config_manager.config();
        
        // Validate project path exists if specified
        if let Some(ref path) = self.project_path {
            if !path.exists() {
                return Err(anyhow::anyhow!(
                    "Project path does not exist: {}", 
                    path.display()
                ));
            }
        }

        Ok(())
    }
}

/// Factory for creating CliContext instances
pub struct CliContextFactory;

impl CliContextFactory {
    /// Create a new CliContext with automatic configuration discovery
    pub fn create(project_path: Option<PathBuf>, verbose: bool) -> Result<CliContext> {
        let context = CliContext::new(project_path, verbose)?;
        context.validate()?;
        Ok(context)
    }

    /// Create a CliContext for the current working directory
    pub fn create_for_current_dir(verbose: bool) -> Result<CliContext> {
        let current_dir = std::env::current_dir().ok();
        Self::create(current_dir, verbose)
    }

    /// Create a CliContext for global configuration
    pub fn create_global(verbose: bool) -> Result<CliContext> {
        Self::create(None, verbose)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_context_creation() {
        let temp_dir = TempDir::new().unwrap();
        let context = CliContext::new(Some(temp_dir.path().to_path_buf()), false).unwrap();
        
        assert_eq!(context.project_path, Some(temp_dir.path().to_path_buf()));
        assert!(!context.verbose);
        assert!(context.config_manager.config().hooks.enabled);
    }

    #[test]
    fn test_context_validation() {
        // Test with non-existent path
        let non_existent = PathBuf::from("/this/path/does/not/exist");
        let context = CliContext::new(Some(non_existent), false).unwrap();
        assert!(context.validate().is_err());
        
        // Test with valid temp dir
        let temp_dir = TempDir::new().unwrap();
        let context = CliContext::new(Some(temp_dir.path().to_path_buf()), false).unwrap();
        assert!(context.validate().is_ok());
    }

    #[test]
    fn test_context_factory() {
        let temp_dir = TempDir::new().unwrap();
        let context = CliContextFactory::create(Some(temp_dir.path().to_path_buf()), true).unwrap();
        
        assert!(context.verbose);
        assert_eq!(context.project_path, Some(temp_dir.path().to_path_buf()));
    }

    #[test]
    fn test_global_context() {
        let context = CliContextFactory::create_global(false).unwrap();
        assert!(context.project_path.is_none());
        assert!(!context.verbose);
    }
}