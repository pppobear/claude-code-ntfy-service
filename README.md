# Claude Code Ntfy Service

A CLI tool that integrates Claude Code hooks with the ntfy notification service, allowing you to receive real-time notifications about Claude Code activities on your mobile device or desktop.

## Features

- 🔔 **Full Hook Support**: Supports all Claude Code hooks (tool execution, file operations, git operations, etc.)
- 📱 **Ntfy Integration**: Send notifications to any ntfy server (default: ntfy.sh)
- 🎨 **Customizable Templates**: Built-in templates for all hooks with support for custom templates
- 🚀 **Background Daemon**: Non-blocking daemon process with message queue and retry logic
- ⚙️ **Flexible Configuration**: Project-level or global configuration
- 🔧 **Easy Setup**: Simple initialization and configuration commands

## Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/claude-code-ntfy-service.git
cd claude-code-ntfy-service

# Build the project
cargo build --release

# Install binaries (optional)
cargo install --path .
```

## Quick Start

### 1. Initialize Configuration

```bash
# Initialize global configuration
claude-ntfy init --global

# Or initialize project-specific configuration
claude-ntfy init --project .
```

### 2. Configure Claude Code Hooks

Add to your `.claude/settings.json` or `~/.claude/settings.json`:

```json
{
  "hooks": {
    "PreToolUse": "claude-ntfy",
    "PostToolUse": "claude-ntfy",
    "UserPromptSubmit": "claude-ntfy",
    "SessionStart": "claude-ntfy",
    "Stop": "claude-ntfy",
    "Notification": "claude-ntfy",
    "SubagentStop": "claude-ntfy"
  }
}
```

### 3. Start the Daemon (Optional but Recommended)

```bash
# Start daemon in background
claude-ntfy daemon start

# Or run in foreground for debugging
claude-ntfy daemon start --foreground
```

### 4. Subscribe to Notifications

1. Install the ntfy app on your device:
   - Android: [Google Play](https://play.google.com/store/apps/details?id=io.heckel.ntfy)
   - iOS: [App Store](https://apps.apple.com/app/ntfy/id1625396347)
   - Desktop: [Web App](https://ntfy.sh)

2. Subscribe to your topic (default: `claude-code-hooks`)

## Configuration

Configuration is stored in `.claude/ntfy-service/config.toml`:

```toml
[ntfy]
server_url = "https://ntfy.sh"
default_topic = "claude-code-hooks"
default_priority = 3
default_tags = ["claude-code"]
auth_token = "" # Optional, for private servers
timeout_secs = 30

[hooks]
enabled = true
# Configure per-hook topics and priorities
[hooks.topics]
PreToolUse = "claude-code-tools"
PostToolUse = "claude-code-tools"

[hooks.priorities]
PreToolUse = 3
PostToolUse = 3
UserPromptSubmit = 3

[templates]
use_custom = false
# Add custom templates per hook
[templates.custom_templates]
PostToolUse = "Tool {{tool_name}} completed: {{#if success}}✅{{else}}❌{{/if}}"

[daemon]
enabled = true
log_level = "info"
max_queue_size = 1000
retry_attempts = 3
retry_delay_secs = 5
```

## Usage

### CLI Commands

```bash
# Test notification
claude-ntfy test "Hello from Claude Code!" --title "Test" --priority 3

# Configure settings
claude-ntfy config set ntfy.server_url "https://ntfy.example.com"
claude-ntfy config set ntfy.default_topic "my-topic"
claude-ntfy config set ntfy.auth_token "your-token"

# Configure hook-specific settings
claude-ntfy config hook PreToolUse --topic "tool-alerts" --priority 4

# View configuration
claude-ntfy config show

# List available templates
claude-ntfy templates
claude-ntfy templates --show git-commit

# Daemon management
claude-ntfy daemon status
claude-ntfy daemon stop
claude-ntfy daemon reload

# Manual hook processing (for testing)
echo '{"tool_name": "Read", "success": true}' | CLAUDE_HOOK=PostToolUse claude-ntfy hook

# Dry run to see what would be sent
echo '{"message": "test"}' | CLAUDE_HOOK=UserPromptSubmit claude-ntfy hook --dry-run
```

## Available Hooks

The tool supports all official Claude Code hooks:

- `PreToolUse`: Runs before tool calls (can block them)
- `PostToolUse`: Runs after tool calls complete
- `UserPromptSubmit`: When user submits a prompt
- `SessionStart`: When a Claude Code session starts
- `Stop`: When a Claude Code session ends
- `Notification`: General notifications
- `SubagentStop`: When a subagent stops

## Custom Templates

Create custom message templates using Handlebars syntax:

```toml
[templates.custom_templates]
PreToolUse = """
🔧 Starting Tool: {{tool_name}}
{{#if description}}Description: {{description}}{{/if}}
Time: {{timestamp}}
"""

PostToolUse = """
✅ Tool Complete: {{tool_name}}
Status: {{#if success}}Success{{else}}Failed{{/if}}
{{#if duration_ms}}Duration: {{duration_ms}}ms{{/if}}
"""
```

## Advanced Features

### Filtering

Control which hooks are processed:

```toml
[hooks.filters]
# Ignore specific tools
PreToolUse = ["!Read", "!Grep"]
# Only notify for specific tools
PostToolUse = ["Write", "Edit"]
```

### Multiple Topics

Route different hooks to different ntfy topics:

```toml
[hooks.topics]
PreToolUse = "claude-tools-start"
PostToolUse = "claude-tools-complete"
UserPromptSubmit = "claude-prompts"
SessionStart = "claude-sessions"
```

### Priority Levels

Set notification priorities (1-5):

```toml
[hooks.priorities]
Stop = 5           # Urgent - Session ended
Notification = 4   # High - Important notification
PostToolUse = 3    # Normal - Tool completed
PreToolUse = 2     # Low - Tool starting
SessionStart = 1   # Min - Session started
```

## Troubleshooting

### Daemon Not Running

```bash
# Check daemon status
claude-ntfy daemon status

# View daemon logs
tail -f ~/.claude/ntfy-service/daemon.log

# Restart daemon
claude-ntfy daemon stop
claude-ntfy daemon start
```

### Notifications Not Received

1. Check configuration:
   ```bash
   claude-ntfy config show
   ```

2. Test notification directly:
   ```bash
   claude-ntfy test "Test message"
   ```

3. Check if hook is filtered:
   ```bash
   claude-ntfy config get hooks.enabled
   ```

4. Run without daemon to see errors:
   ```bash
   echo '{"test": "data"}' | CLAUDE_HOOK=test claude-ntfy --no-daemon
   ```

## Environment Variables

- `CLAUDE_HOOK`: Hook name (automatically set by Claude Code)
- `CLAUDE_TOOL`: Current tool being used
- `CLAUDE_PROJECT_DIR`: Absolute path to the project root directory
- `CLAUDE_WORKSPACE`: Workspace path
- `CLAUDE_TOOL_NAME`: Specific tool name
- `CLAUDE_TOOL_STATUS`: Tool execution status
- `CLAUDE_TOOL_DURATION`: Tool execution duration

## Exit Codes

The tool follows Claude Code hook conventions:
- Exit code 0: Success, hook executed normally
- Exit code 2: Blocking error, prevents the action from continuing
- Exit code 1: General error (non-blocking)

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Acknowledgments

- [Claude Code](https://claude.ai/code) by Anthropic
- [ntfy](https://ntfy.sh) by Philipp Heckel