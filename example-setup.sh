#!/bin/bash
# Example setup script for Claude Code Ntfy Service

echo "🚀 Setting up Claude Code Ntfy Service..."

# Build the project
echo "📦 Building the project..."
cargo build --release

# Create symbolic links for easier access (optional)
echo "🔗 Creating symbolic links..."
sudo ln -sf "$(pwd)/target/release/claude-ntfy" /usr/local/bin/claude-ntfy
sudo ln -sf "$(pwd)/target/release/claude-ntfy-daemon" /usr/local/bin/claude-ntfy-daemon

# Initialize configuration
echo "⚙️ Initializing configuration..."
claude-ntfy init --global

# Show example Claude Code settings
echo ""
echo "📝 Add the following to your .claude/settings.json or ~/.claude/settings.json:"
echo ""
cat <<EOF
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "claude-ntfy --project \$CLAUDE_PROJECT_DIR"
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
            "command": "claude-ntfy --project \$CLAUDE_PROJECT_DIR"
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "claude-ntfy --project \$CLAUDE_PROJECT_DIR"
          }
        ]
      }
    ],
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "claude-ntfy --project \$CLAUDE_PROJECT_DIR"
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "claude-ntfy --project \$CLAUDE_PROJECT_DIR"
          }
        ]
      }
    ],
    "Notification": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "claude-ntfy --project \$CLAUDE_PROJECT_DIR"
          }
        ]
      }
    ],
    "SubagentStop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "claude-ntfy --project \$CLAUDE_PROJECT_DIR"
          }
        ]
      }
    ]
  }
}
EOF

echo ""
echo "🎯 Next steps:"
echo "1. Subscribe to 'claude-code-hooks' topic in your ntfy app"
echo "2. Start the daemon: claude-ntfy daemon start"
echo "3. Test with: claude-ntfy test 'Hello from Claude Code!'"
echo ""
echo "✅ Setup complete!"