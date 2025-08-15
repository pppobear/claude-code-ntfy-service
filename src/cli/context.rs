//! CLI Context for dependency injection and shared state
//! 
//! This module provides the CliContext abstraction that centralizes
//! configuration management and reduces coupling in CLI handlers.

use std::path::PathBuf;
use std::sync::Arc;
use anyhow::Result;
use crate::config::ConfigManager;

/// CLI execution context containing shared dependencies and configuration
#[derive(Clone)]
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
        // Test with non-existent path - should fail during construction
        let non_existent = PathBuf::from("/this/path/does/not/exist");
        let context_result = CliContext::new(Some(non_existent), false);
        assert!(context_result.is_err());
        
        // Test with valid temp dir
        let temp_dir = TempDir::new().unwrap();
        let context = CliContext::new(Some(temp_dir.path().to_path_buf()), false).unwrap();
        // Validation removed - just check that context was created successfully
        assert!(context.config_manager.config().hooks.enabled);
    }

    #[test]
    fn test_context_verbose_mode() {
        let temp_dir = TempDir::new().unwrap();
        let context = CliContext::new(Some(temp_dir.path().to_path_buf()), true).unwrap();
        
        assert!(context.verbose);
        assert_eq!(context.project_path, Some(temp_dir.path().to_path_buf()));
    }

    #[test]
    fn test_global_context() {
        let context = CliContext::new(None, false).unwrap();
        assert!(context.project_path.is_none());
        assert!(!context.verbose);
    }
}