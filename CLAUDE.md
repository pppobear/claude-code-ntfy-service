# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Claude Code Ntfy Service** is a Rust CLI tool that integrates Claude Code hooks with ntfy notification services. It allows developers to receive real-time notifications about Claude Code activities on mobile devices or desktop.

### Architecture

- **CLI Frontend** (`src/main.rs`): Main command interface with subcommands for config, daemon management, testing
- **Background Daemon** (`src/daemon.rs`): Async daemon process for non-blocking notification processing
- **Configuration Manager** (`src/config.rs`): Handles TOML-based configuration with project/global scopes
- **Ntfy Client** (`src/ntfy.rs`): HTTP client for sending notifications to ntfy servers
- **Template Engine** (`src/templates.rs`): Handlebars-based message formatting system

### Key Components

- **Hook Processing**: Supports all Claude Code hooks (PreToolUse, PostToolUse, UserPromptSubmit, SessionStart, Stop, etc.)
- **Daemon Architecture**: Background processing with retry logic, graceful shutdown, and IPC via command files
- **Template System**: Customizable message templates with built-in defaults for each hook type
- **Configuration**: Project-level (`.claude/ntfy-service/config.toml`) and global (`~/.claude/ntfy-service/config.toml`) support

## Development Commands

### Build and Test
```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Integration tests specifically
cargo test --test integration_test

# Run with debug logging
RUST_LOG=debug cargo run -- <commands>
```

### Development Workflow
```bash
# Build and install locally for testing
cargo install --path .

# Test CLI functionality
./target/debug/claude-ntfy --help
./target/debug/claude-ntfy test "Hello World" --title "Test"

# Test daemon functionality
./target/debug/claude-ntfy daemon start
./target/debug/claude-ntfy daemon status
./target/debug/claude-ntfy daemon stop
```

### Hook Testing
```bash
# Test hook processing with sample data
echo '{"tool_name": "Read", "success": true}' | CLAUDE_HOOK=PostToolUse ./target/debug/claude-ntfy hook --dry-run

# Test daemon communication
./target/debug/claude-ntfy daemon start -d
echo '{"tool_name": "Write", "file_path": "/tmp/test.txt"}' | CLAUDE_HOOK=PreToolUse ./target/debug/claude-ntfy
```

## Configuration

### Project Structure
```
.claude/ntfy-service/
├── config.toml          # Main configuration
├── daemon.sock          # IPC socket (when daemon running)  
├── daemon.pid           # Daemon process ID
├── daemon.cmd           # Daemon command file
└── daemon.log           # Daemon logs
```

### Configuration Management
```bash
# Initialize configuration
claude-ntfy init --project .     # Project-level
claude-ntfy init --global        # Global level

# Configure ntfy settings
claude-ntfy config set ntfy.server_url "https://ntfy.sh"
claude-ntfy config set ntfy.default_topic "my-topic"
claude-ntfy config set ntfy.auth_token "your-token"

# Configure hook-specific settings
claude-ntfy config hook PreToolUse --topic "tool-alerts" --priority 4
```

## Claude Code Integration

### Hook Configuration
Add to `.claude/settings.json` or `~/.claude/settings.json`:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command", 
            "command": "claude-ntfy"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "claude-ntfy"
          }
        ]
      }
    ]
  }
}
```

### Environment Variables
The tool uses these Claude Code environment variables:
- `CLAUDE_HOOK`: Hook event name (automatically set)
- `CLAUDE_TOOL`: Current tool name
- `CLAUDE_PROJECT_DIR`: Project root directory
- `CLAUDE_TOOL_NAME`: Specific tool name
- `CLAUDE_TOOL_STATUS`: Tool execution status

## Implementation Details

### Daemon Architecture
- Uses tokio async runtime with graceful shutdown
- IPC via filesystem (command files) rather than sockets for portability
- Retry logic with exponential backoff for failed notifications
- Automatic PID file management and process monitoring

### Hook Data Processing
- Enhances PostToolUse data to ensure `tool_response.success` field exists
- Supports both JSON input from stdin and environment variable fallback
- Template engine formats hook data for notification messages

### Error Handling
- Exit code 0: Success (hooks continue)
- Exit code 2: Blocking error (prevents tool execution) 
- Exit code 1: General error (non-blocking)

### Binary Structure
- `claude-ntfy`: Main CLI binary
- `claude-ntfy-daemon`: Daemon process binary
- Both share common modules for config, ntfy client, templates

### Testing Approach
- Integration tests using `assert_cmd` for CLI testing
- Tests cover initialization, configuration, hook processing
- Dry-run mode for testing without sending notifications