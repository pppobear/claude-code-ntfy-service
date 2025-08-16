mod common;

use common::{TestEnvironment, helpers, assertions};

#[test]
fn test_cli_help() {
    helpers::test_help_contains("Claude Code hook CLI tool");
}

#[test]
fn test_cli_version() {
    helpers::test_version_contains("claude-ntfy");
}

#[test]
fn test_init_command() {
    let env = TestEnvironment::new();

    env.init_config().done();

    // Check that config file was created
    assertions::assert_path_exists(&env.config_path);
}

#[test]
fn test_hook_dry_run() {
    helpers::test_hook_dry_run(
        "PostToolUse",
        r#"{"tool_name": "Read", "success": true}"#,
        "Dry run - would send notification",
    );
}

#[test]
fn test_templates_list() {
    helpers::test_templates_list_contains(&[
        "PostToolUse",
        "PreToolUse", 
        "generic"
    ]);
}

#[test]
fn test_config_show() {
    let env = TestEnvironment::new();

    // First initialize config
    env.init_config().done();

    // Then show config
    env.command()
        .args(["config", "show"])
        .expect_success()
        .stdout_contains_all(&["[ntfy]", "server_url"])
        .done();
}

#[test]
fn test_daemon_status() {
    helpers::test_daemon_not_running();
}
