use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

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
        .stdout(predicate::str::contains("Available templates"))
        .stdout(predicate::str::contains("PostToolUse"))
        .stdout(predicate::str::contains("PreToolUse"))
        .stdout(predicate::str::contains("generic"));
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
fn test_daemon_status() {
    let mut cmd = Command::cargo_bin("claude-ntfy").unwrap();

    cmd.arg("daemon")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("Daemon is not running"));
}
