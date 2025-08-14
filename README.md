# Claude Code Ntfy Service

A Rust CLI tool that integrates Claude Code hooks with the ntfy notification service, enabling real-time notifications about Claude Code activities on your mobile device or desktop.

## Features

- 🔔 **Full Hook Support**: Supports all Claude Code hooks (PreToolUse, PostToolUse, UserPromptSubmit, SessionStart, Stop, Notification, SubagentStop)
- 📱 **Ntfy Integration**: Send notifications to any ntfy server (default: ntfy.sh) with both text and JSON formats
- 🎨 **Rich Templates**: Built-in Handlebars templates for all hooks with custom template support
- 🚀 **Background Daemon**: Async daemon process with retry logic, file-based IPC, and graceful shutdown
- ⚙️ **Dual Configuration**: Project-level (`.claude/ntfy-service/`) or global (`~/.claude/ntfy-service/`) configuration
- 🔧 **Advanced Features**: Hook filtering, multiple topics, priority levels, and enhanced PostToolUse data processing
- 📝 **Comprehensive Logging**: File and console logging with configurable levels

## Architecture

**Two-Binary Design**:
- **`claude-ntfy`**: Main CLI tool for configuration, testing, and daemon management
- **`claude-ntfy-daemon`**: Background daemon process for async notification processing

**Communication**: Unix socket IPC system with binary serialization for high-performance communication.

## Installation

```bash
# Clone the repository
git clone https://github.com/pppobear/claude-code-ntfy-service.git
cd claude-code-ntfy-service

# Build both binaries
cargo build --release

# Install both binaries system-wide (recommended)
sudo cp target/release/claude-ntfy /usr/local/bin/
sudo cp target/release/claude-ntfy-daemon /usr/local/bin/

# Or use the example setup script
chmod +x example-setup.sh
./example-setup.sh
```

## Quick Start

### 1. Initialize Configuration

```bash
# Initialize global configuration
claude-ntfy init --global

# Or initialize project-specific configuration  
claude-ntfy init

# Force overwrite existing configuration
claude-ntfy init --global --force
```

### 2. Configure Claude Code Hooks

Add to your `.claude/settings.json` or `~/.claude/settings.json`:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "claude-ntfy --project $CLAUDE_PROJECT_DIR"
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
            "command": "claude-ntfy --project $CLAUDE_PROJECT_DIR"
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "claude-ntfy --project $CLAUDE_PROJECT_DIR"
          }
        ]
      }
    ],
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "claude-ntfy --project $CLAUDE_PROJECT_DIR"
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "claude-ntfy --project $CLAUDE_PROJECT_DIR"
          }
        ]
      }
    ],
    "Notification": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "claude-ntfy --project $CLAUDE_PROJECT_DIR"
          }
        ]
      }
    ],
    "SubagentStop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "claude-ntfy --project $CLAUDE_PROJECT_DIR"
          }
        ]
      }
    ]
  }
}
```

### 3. Start the Daemon

```bash
# Start daemon in foreground (default - good for debugging)
claude-ntfy daemon start

# Start daemon in background (detached)
claude-ntfy daemon start -d
claude-ntfy daemon start --detach
```

### 4. Subscribe to Notifications

1. Install the ntfy app on your device:
   - Android: [Google Play](https://play.google.com/store/apps/details?id=io.heckel.ntfy)
   - iOS: [App Store](https://apps.apple.com/app/ntfy/id1625396347)
   - Desktop: [Web App](https://ntfy.sh)

2. Subscribe to your topic (default: `claude-code-hooks`)

## Configuration

Configuration is stored in `.claude/ntfy-service/config.toml` (project-level) or `~/.claude/ntfy-service/config.toml` (global):

```toml
[ntfy]
server_url = "https://ntfy.sh"
default_topic = "claude-code-hooks"
default_priority = 3                    # Optional: 1-5
default_tags = ["claude-code"]          # Optional: array of strings
auth_token = ""                         # Optional: for private servers
timeout_secs = 30                       # Optional: HTTP timeout
send_format = "text"                    # "text" or "json"

[hooks]
enabled = true
# Per-hook topic routing
[hooks.topics]
PreToolUse = "claude-tools-start"
PostToolUse = "claude-tools-complete"
UserPromptSubmit = "claude-prompts"

# Per-hook priority levels
[hooks.priorities]
PreToolUse = 2
PostToolUse = 3
UserPromptSubmit = 3
Stop = 5

# Hook filtering (inclusion/exclusion)
[hooks.filters]
PreToolUse = ["!Read", "!Grep"]         # Exclude Read and Grep tools
PostToolUse = ["Write", "Edit"]         # Only notify for Write and Edit

[templates]
use_custom = false
variables = {}                          # Custom template variables

# Custom templates using Handlebars syntax
[templates.custom_templates]
PreToolUse = """
🔧 Starting Tool: {{tool_name}}
{{#if tool_input.file_path}}File: {{tool_input.file_path}}{{/if}}
Time: {{timestamp}}
"""

PostToolUse = """
{{#if tool_response.error}}❌{{else}}✅{{/if}} Tool: {{tool_name}}
{{#if tool_response.filePath}}File: {{tool_response.filePath}}{{/if}}
{{#if tool_response.error}}Error: {{tool_response.error}}{{/if}}
Status: {{#if tool_response.error}}Failed{{else}}Success{{/if}}
"""

[daemon]
enabled = true
socket_path = ""                        # Optional: custom socket path  
log_level = "info"                      # trace, debug, info, warn, error
log_path = ""                           # Optional: file logging path
max_queue_size = 1000
retry_attempts = 3
retry_delay_secs = 5
```

## CLI Commands

### Initialization
```bash
claude-ntfy init [--global] [--force]
```

### Configuration Management
```bash
# View current configuration
claude-ntfy config show

# Set configuration values
claude-ntfy config set ntfy.server_url "https://ntfy.example.com"
claude-ntfy config set ntfy.default_topic "my-topic"
claude-ntfy config set ntfy.auth_token "your-token"
claude-ntfy config set daemon.enabled true
claude-ntfy config set daemon.log_path "/path/to/daemon.log"

# Get configuration values
claude-ntfy config get ntfy.server_url
claude-ntfy config get daemon.enabled
claude-ntfy config get daemon.log_path

# Configure hook-specific settings
claude-ntfy config hook PreToolUse --topic "tool-alerts" --priority 4
claude-ntfy config hook PostToolUse --priority 3 --filter "Write"
```

### Daemon Management
```bash
# Check daemon status
claude-ntfy daemon status

# Stop daemon
claude-ntfy daemon stop

# Reload daemon configuration
claude-ntfy daemon reload
```

### Testing
```bash
# Test notification sending
claude-ntfy test "Hello from Claude Code!" --title "Test Notification"
claude-ntfy test "Custom message" --title "Test" --priority 5 --topic "test-topic"
```

### Templates
```bash
# List available templates
claude-ntfy templates

# Show specific template content
claude-ntfy templates --show PreToolUse
claude-ntfy templates --show PostToolUse
```

### Hook Processing (Advanced)
```bash
# Manual hook processing (for testing)
echo '{"tool_name": "Read", "success": true}' | CLAUDE_HOOK=PostToolUse claude-ntfy hook

# Process hook directly (bypass daemon)
echo '{"tool_name": "Write"}' | CLAUDE_HOOK=PreToolUse claude-ntfy hook --no-daemon

# Dry run to see what would be sent
echo '{"message": "test"}' | CLAUDE_HOOK=UserPromptSubmit claude-ntfy hook --dry-run
```

## Available Hooks

The tool supports all official Claude Code hooks:

- **`PreToolUse`**: Runs before tool calls (can block execution if exit code 2)
- **`PostToolUse`**: Runs after tool calls complete
- **`UserPromptSubmit`**: When user submits a prompt  
- **`SessionStart`**: When a Claude Code session starts
- **`Stop`**: When a Claude Code session ends
- **`Notification`**: General notifications from Claude Code
- **`SubagentStop`**: When a subagent stops

## Built-in Templates

Each hook has a carefully crafted template optimized for mobile notifications:

### PreToolUse
```
🔧 Tool Starting: {{tool_name}}
File: {{tool_input.file_path}}
Command: {{tool_input.command}}
Time: {{timestamp}}
```

### PostToolUse  
```
✅ Tool Completed: {{tool_name}}
Status: {{#if tool_response.error}}Failed{{else}}Success{{/if}}
File: {{tool_response.filePath}}
Error: {{tool_response.error}}
Duration: {{duration_ms}}ms
Time: {{timestamp}}
```

### UserPromptSubmit
```
💬 User Prompt
Message: {{prompt}}
Session: {{session_id}}
Time: {{timestamp}}
```

### SessionStart
```
🚀 Claude Code Session Started
Session: {{session_id}}
Working Dir: {{cwd}}
Source: {{source}}
Time: {{timestamp}}
```

### Stop
```
🏁 Claude Code Session Ended
Session: {{session_id}}
Time: {{timestamp}}
```

## Advanced Features

### Hook Filtering

Control which notifications are sent:

```toml
[hooks.filters]
# Exclude specific tools (prefix with !)
PreToolUse = ["!Read", "!Grep", "!LS"]

# Only notify for specific tools
PostToolUse = ["Write", "Edit", "MultiEdit"]

# Combine inclusion and exclusion
UserPromptSubmit = ["deploy", "!debug"]
```

### Multiple Topics

Route different hooks to different ntfy topics:

```toml
[hooks.topics]
PreToolUse = "claude-tools-start"
PostToolUse = "claude-tools-complete"  
UserPromptSubmit = "claude-prompts"
SessionStart = "claude-sessions"
Stop = "claude-sessions"
Notification = "claude-alerts"
SubagentStop = "claude-subagents"
```

### Priority Levels

Set notification priorities (1-5, where 5 is highest):

```toml
[hooks.priorities]
Stop = 5              # Max - Session ended
Notification = 4      # High - Important alerts
PostToolUse = 3       # Default - Tool completed
PreToolUse = 2        # Low - Tool starting
SessionStart = 1      # Min - Session started
```

### Send Formats

Choose between text and JSON sending modes:

```toml
[ntfy]
send_format = "text"    # Default - uses HTTP headers, better compatibility
# send_format = "json"  # Alternative - structured JSON body
```

## Daemon Architecture

### Process Management
- **PID Files**: `.claude/ntfy-service/daemon.pid` 
- **Socket Files**: `.claude/ntfy-service/daemon.sock` (Unix socket IPC)

### Logging Options
```bash
# Console only (foreground mode)
claude-ntfy daemon start

# File logging (background mode) 
claude-ntfy config set daemon.log_path "/path/to/daemon.log"
claude-ntfy daemon start -d

# Dual logging (foreground with file backup)
claude-ntfy config set daemon.log_path "/path/to/daemon.log" 
claude-ntfy daemon start
```

### IPC Communication
The daemon uses Unix socket IPC for high-performance communication:
1. CLI connects to Unix socket at `.claude/ntfy-service/daemon.sock`
2. Messages are serialized using bincode for efficiency
3. Bidirectional communication with length-prefixed protocol
4. Supports: Submit, Shutdown, Reload, Status, Ping

## Troubleshooting

### Daemon Not Starting
```bash
# Check for existing daemon
claude-ntfy daemon status

# View daemon logs
tail -f ~/.claude/ntfy-service/daemon.log
# or project-specific logs:
tail -f .claude/ntfy-service/daemon.log

# Clean up stale PID files
claude-ntfy daemon stop
rm -f .claude/ntfy-service/daemon.pid

# Start with verbose logging
RUST_LOG=debug claude-ntfy daemon start
```

### Notifications Not Received

1. **Verify configuration**:
   ```bash
   claude-ntfy config show
   ```

2. **Test notification directly**:
   ```bash
   claude-ntfy test "Test message" --title "Direct Test"
   ```

3. **Check hook filtering**:
   ```bash
   claude-ntfy config get hooks.enabled
   claude-ntfy config show | grep -A5 "\[hooks.filters\]"
   ```

4. **Test hook processing**:
   ```bash
   echo '{"tool_name": "Write", "success": true}' | \
   CLAUDE_HOOK=PostToolUse claude-ntfy hook --dry-run
   ```

5. **Bypass daemon for debugging**:
   ```bash
   echo '{"test": "data"}' | \
   CLAUDE_HOOK=UserPromptSubmit claude-ntfy hook --no-daemon
   ```

### Common Issues

**"Daemon already running" error**:
```bash
claude-ntfy daemon stop
# Wait a moment, then:
claude-ntfy daemon start -d
```

**"Failed to send notification" errors**:
- Check internet connection
- Verify ntfy server URL: `claude-ntfy config get ntfy.server_url`
- Test with a simple message: `claude-ntfy test "hello"`
- Check auth token if using private server

**Hook not triggering**:
- Verify Claude Code settings.json syntax
- Check that `claude-ntfy` binary is in PATH
- Test with verbose logging: `claude-ntfy -v daemon start`

## Environment Variables

The tool automatically reads these Claude Code environment variables:

- **`CLAUDE_HOOK`**: Hook event name (automatically set by Claude Code)
- **`CLAUDE_TOOL`**: Current tool being used  
- **`CLAUDE_PROJECT_DIR`**: Absolute path to project root directory
- **`CLAUDE_WORKSPACE`**: Workspace path
- **`CLAUDE_TOOL_NAME`**: Specific tool name
- **`CLAUDE_TOOL_STATUS`**: Tool execution status
- **`CLAUDE_TOOL_DURATION`**: Tool execution duration

Additional environment variables:
- **`RUST_LOG`**: Set logging level (trace, debug, info, warn, error)

## Exit Codes

Following Claude Code hook conventions:
- **Exit code 0**: Success, hook executed normally
- **Exit code 2**: Blocking error, prevents the tool from executing (PreToolUse only)
- **Exit code 1**: General error (non-blocking)

## Integration Examples

### Project-Specific Setup
```bash
# In your project directory
claude-ntfy init
claude-ntfy config set ntfy.default_topic "myproject-claude"
claude-ntfy daemon start -d
```

### Global Setup with Custom Server
```bash
claude-ntfy init --global
claude-ntfy config set ntfy.server_url "https://ntfy.mycompany.com"
claude-ntfy config set ntfy.auth_token "tk_mytoken123"
claude-ntfy config set ntfy.default_topic "dev-claude-notifications"
claude-ntfy daemon start -d
```

### Development Setup with Filtering
```bash
# Only notify for file operations, not reads
claude-ntfy config hook PreToolUse --filter "Write" --filter "Edit" --filter "MultiEdit"
claude-ntfy config hook PostToolUse --topic "dev-file-changes" --priority 4
```

## Performance & Resources

- **Memory Usage**: ~5-10MB for daemon process
- **CPU Usage**: Minimal when idle, brief spikes during notification processing  
- **Network**: Only when sending notifications (HTTP POST requests)
- **Disk I/O**: Minimal for configuration and IPC files
- **Latency**: <100ms from hook trigger to notification sent (local processing)

## Security Considerations

- **Auth Tokens**: Stored in plain text in config files - ensure proper file permissions
- **Network**: Uses HTTPS by default for ntfy.sh
- **IPC**: File-based IPC is readable by same user - consider file permissions in shared environments
- **PID Files**: Standard Unix conventions for daemon process management

## License

MIT License - see LICENSE file for details.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass: `cargo test`
5. Submit a pull request

## Acknowledgments

- [Claude Code](https://claude.ai/code) by Anthropic for the amazing AI-powered development experience
- [ntfy](https://ntfy.sh) by Philipp Heckel for the excellent notification service
- Rust community for the fantastic ecosystem of crates used in this project

## Changelog

### v0.1.0 (Initial Release)
- Full Claude Code hook support for all 7 official hooks
- Background daemon with async processing and retry logic
- Rich Handlebars templating system with built-in templates
- Comprehensive configuration management with project/global scopes
- Advanced features: filtering, multiple topics, priority levels
- File-based IPC system with PID management
- Dual-format support (text/JSON) for ntfy compatibility
- Enhanced PostToolUse data processing for better success detection
- Cross-platform compatibility (Unix/Windows)
- Extensive CLI with 20+ commands and options
- Comprehensive logging with file and console output
- Integration tests and example setup scripts