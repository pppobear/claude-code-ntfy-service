//! Common test utilities and helpers
//! 
//! This module provides reusable test helpers to reduce code duplication
//! across integration tests.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;
use tempfile::TempDir;

/// Test command builder for claude-ntfy CLI
pub struct TestCommand {
    cmd: Command,
}

impl TestCommand {
    /// Create a new test command for claude-ntfy binary
    pub fn new() -> Self {
        let cmd = Command::cargo_bin("claude-ntfy")
            .expect("Failed to find claude-ntfy binary");
        Self { cmd }
    }
    
    /// Add arguments to the command
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for arg in args {
            self.cmd.arg(arg.as_ref());
        }
        self
    }
    
    /// Add a single argument to the command
    pub fn arg<S: AsRef<str>>(mut self, arg: S) -> Self {
        self.cmd.arg(arg.as_ref());
        self
    }
    
    /// Set environment variable
    pub fn env<K, V>(mut self, key: K, val: V) -> Self
    where
        K: AsRef<str>,
        V: AsRef<str>,
    {
        self.cmd.env(key.as_ref(), val.as_ref());
        self
    }
    
    /// Write stdin input
    pub fn stdin<S: AsRef<str>>(mut self, input: S) -> Self {
        self.cmd.write_stdin(input.as_ref());
        self
    }
    
    /// Execute and expect success
    pub fn expect_success(mut self) -> TestAssertion {
        let assert = self.cmd.assert().success();
        TestAssertion { assert }
    }
    
}

impl Default for TestCommand {
    fn default() -> Self {
        Self::new()
    }
}

/// Test assertion wrapper with convenient methods
pub struct TestAssertion {
    assert: assert_cmd::assert::Assert,
}

impl TestAssertion {
    /// Assert stdout contains text
    pub fn stdout_contains<S: AsRef<str>>(self, text: S) -> Self {
        let assert = self.assert.stdout(predicate::str::contains(text.as_ref()));
        Self { assert }
    }
    
    
    /// Assert multiple stdout patterns
    pub fn stdout_contains_all<I, S>(mut self, patterns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for pattern in patterns {
            self.assert = self.assert.stdout(predicate::str::contains(pattern.as_ref()));
        }
        Self { assert: self.assert }
    }
    
    /// Finish the assertion
    pub fn done(self) -> assert_cmd::assert::Assert {
        self.assert
    }
}

/// Test environment setup helper
pub struct TestEnvironment {
    pub temp_dir: TempDir,
    pub config_path: std::path::PathBuf,
}

impl TestEnvironment {
    /// Create a new test environment with temporary directory
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_path = temp_dir.path().join(".claude/ntfy-service/config.toml");
        
        Self {
            temp_dir,
            config_path,
        }
    }
    
    /// Initialize configuration in the test environment
    pub fn init_config(&self) -> TestAssertion {
        TestCommand::new()
            .args(["init", "--project"])
            .arg(self.temp_dir.path().to_string_lossy().as_ref())
            .expect_success()
    }
    
    /// Get the project path
    pub fn project_path(&self) -> &Path {
        self.temp_dir.path()
    }
    
    
    /// Create a command configured for this environment
    pub fn command(&self) -> TestCommand {
        TestCommand::new()
            .arg("--project")
            .arg(self.project_path().to_string_lossy().as_ref())
    }
}

impl Default for TestEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper functions for common test operations
pub mod helpers {
    use super::*;
    
    /// Test help command output
    pub fn test_help_contains(expected_text: &str) {
        TestCommand::new()
            .arg("--help")
            .expect_success()
            .stdout_contains(expected_text)
            .done();
    }
    
    /// Test version command output
    pub fn test_version_contains(expected_text: &str) {
        TestCommand::new()
            .arg("--version")
            .expect_success()
            .stdout_contains(expected_text)
            .done();
    }
    
    /// Test daemon status (expecting not running)
    pub fn test_daemon_not_running() {
        TestCommand::new()
            .args(["daemon", "status"])
            .expect_success()
            .stdout_contains("Daemon is not running")
            .done();
    }
    
    /// Test templates list command
    pub fn test_templates_list_contains(expected_templates: &[&str]) {
        let assertion = TestCommand::new()
            .arg("templates")
            .expect_success()
            .stdout_contains("Available templates");
            
        assertion
            .stdout_contains_all(expected_templates)
            .done();
    }
    
    /// Test hook with dry run
    pub fn test_hook_dry_run(hook_name: &str, stdin_data: &str, expected_output: &str) {
        TestCommand::new()
            .arg("hook")
            .arg("--dry-run")
            .env("CLAUDE_HOOK", hook_name)
            .stdin(stdin_data)
            .expect_success()
            .stdout_contains(expected_output)
            .stdout_contains(hook_name)
            .done();
    }
}

/// Assertion helpers for common patterns
pub mod assertions {
    /// Assert that a path exists
    pub fn assert_path_exists<P: AsRef<std::path::Path>>(path: P) {
        assert!(
            path.as_ref().exists(),
            "Path should exist: {}",
            path.as_ref().display()
        );
    }
    
}