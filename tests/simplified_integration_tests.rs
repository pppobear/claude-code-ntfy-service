//! Simplified Integration Tests
//! 
//! Focused integration tests that validate core functionality
//! without complex dependencies

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use tempfile::TempDir;

#[cfg(test)]
mod basic_integration_tests {
    use super::*;

    #[test]
    fn test_cli_help() {
        let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();
        cmd.arg("--help")
            .assert()
            .success()
            .stdout(predicate::str::contains("Claude Code hook CLI tool"));
    }

    #[test]
    fn test_cli_version() {
        let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();
        cmd.arg("--version")
            .assert()
            .success()
            .stdout(predicate::str::contains("claude-ntfy"));
    }

    #[test]
    fn test_init_command() {
        let temp_dir = TempDir::new().unwrap();
        let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();

        cmd.arg("init")
            .arg("--project")
            .arg(temp_dir.path())
            .assert()
            .success();

        // Check that config file was created
        assert!(temp_dir
            .path()
            .join(".claude/ntfy-service/config.toml")
            .exists());
    }

    #[test]
    fn test_hook_dry_run() {
        let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();

        cmd.arg("hook")
            .arg("--dry-run")
            .env("CLAUDE_HOOK", "PostToolUse")
            .write_stdin(r#"{"tool_name": "Read", "success": true}"#)
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "Dry run - would send notification",
            ))
            .stdout(predicate::str::contains("PostToolUse"));
    }

    #[test]
    fn test_templates_list() {
        let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();

        cmd.arg("templates")
            .assert()
            .success()
            .stdout(predicate::str::contains("Available templates"));
    }

    #[test]
    fn test_config_show() {
        let temp_dir = TempDir::new().unwrap();
        let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();

        // First initialize config
        cmd.arg("init")
            .arg("--project")
            .arg(temp_dir.path())
            .assert()
            .success();

        // Then show config
        let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();
        cmd.arg("config")
            .arg("show")
            .arg("--project")
            .arg(temp_dir.path())
            .assert()
            .success()
            .stdout(predicate::str::contains("[ntfy]"))
            .stdout(predicate::str::contains("server_url"));
    }

    #[test]
    fn test_config_set_get() {
        let temp_dir = TempDir::new().unwrap();
        
        // Initialize config
        let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();
        cmd.arg("init")
            .arg("--project")
            .arg(temp_dir.path())
            .assert()
            .success();

        // Set a value
        let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();
        cmd.arg("config")
            .arg("set")
            .arg("ntfy.default_topic")
            .arg("test-topic")
            .arg("--project")
            .arg(temp_dir.path())
            .assert()
            .success();

        // Get the value back
        let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();
        cmd.arg("config")
            .arg("get")
            .arg("ntfy.default_topic")
            .arg("--project")
            .arg(temp_dir.path())
            .assert()
            .success()
            .stdout(predicate::str::contains("test-topic"));
    }

    #[test]
    fn test_daemon_status() {
        let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();

        cmd.arg("daemon")
            .arg("status")
            .assert()
            .success()
            .stdout(predicate::str::contains("Daemon is not running"));
    }

    #[test]
    fn test_hook_configuration() {
        let temp_dir = TempDir::new().unwrap();
        
        // Initialize config
        let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();
        cmd.arg("init")
            .arg("--project")
            .arg(temp_dir.path())
            .assert()
            .success();

        // Configure a hook
        let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();
        cmd.arg("config")
            .arg("hook")
            .arg("PostToolUse")
            .arg("--topic")
            .arg("tool-notifications")
            .arg("--priority")
            .arg("4")
            .arg("--project")
            .arg(temp_dir.path())
            .assert()
            .success();
    }

    #[test]
    fn test_end_to_end_workflow() {
        let temp_dir = TempDir::new().unwrap();

        // Step 1: Initialize project
        let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();
        cmd.arg("init")
            .arg("--project")
            .arg(temp_dir.path())
            .assert()
            .success();

        // Step 2: Configure hooks
        let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();
        cmd.arg("config")
            .arg("hook")
            .arg("PostToolUse")
            .arg("--topic")
            .arg("claude-tools")
            .arg("--priority")
            .arg("3")
            .arg("--project")
            .arg(temp_dir.path())
            .assert()
            .success();

        // Step 3: Test hook processing (dry run)
        let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();
        cmd.arg("hook")
            .arg("--dry-run")
            .arg("--project")
            .arg(temp_dir.path())
            .env("CLAUDE_HOOK", "PostToolUse")
            .env("CLAUDE_TOOL_NAME", "Read")
            .env("CLAUDE_TOOL_STATUS", "success")
            .write_stdin(r#"{"tool_name": "Read", "success": true, "duration_ms": "42"}"#)
            .assert()
            .success()
            .stdout(predicate::str::contains("PostToolUse"))
            .stdout(predicate::str::contains("Read"));

        // Step 4: Verify templates work
        let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();
        cmd.arg("templates")
            .arg("--show")
            .arg("PostToolUse")
            .assert()
            .success();
    }
}

/// Performance validation tests
#[cfg(test)]
mod performance_validation_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_hook_processing_performance() {
        let temp_dir = TempDir::new().unwrap();
        
        // Initialize config
        let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();
        cmd.arg("init")
            .arg("--project")
            .arg(temp_dir.path())
            .assert()
            .success();

        // Measure time for multiple hook operations
        let start = Instant::now();
        
        for i in 0..10 {
            let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();
            cmd.arg("hook")
                .arg("--dry-run")
                .arg("--project")
                .arg(temp_dir.path())
                .env("CLAUDE_HOOK", "PostToolUse")
                .write_stdin(&format!(r#"{{"tool_name": "Read", "success": true, "index": {}}}"#, i))
                .assert()
                .success();
        }
        
        let duration = start.elapsed();
        println!("Processed 10 hooks in {:?}", duration);
        
        // Performance assertion - should be much faster than old polling system
        assert!(duration.as_millis() < 5000, "Hook processing should be fast, took {:?}", duration);
    }

    #[test]
    fn test_config_operations_performance() {
        let temp_dir = TempDir::new().unwrap();
        
        // Initialize config
        let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();
        cmd.arg("init")
            .arg("--project")
            .arg(temp_dir.path())
            .assert()
            .success();

        let start = Instant::now();
        
        // Perform multiple config operations
        for i in 0..5 {
            let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();
            cmd.arg("config")
                .arg("set")
                .arg("ntfy.default_topic")
                .arg(&format!("test-topic-{}", i))
                .arg("--project")
                .arg(temp_dir.path())
                .assert()
                .success();
                
            let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();
            cmd.arg("config")
                .arg("get")
                .arg("ntfy.default_topic")
                .arg("--project")
                .arg(temp_dir.path())
                .assert()
                .success();
        }
        
        let duration = start.elapsed();
        println!("Performed 10 config operations in {:?}", duration);
        
        // Config operations should be fast
        assert!(duration.as_millis() < 2000, "Config operations should be fast, took {:?}", duration);
    }
}