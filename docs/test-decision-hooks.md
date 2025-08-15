# Testing Decision-Requiring Hook Priorities

This document demonstrates how to test the new decision-requiring hook priority system.

## Overview

The claude-ntfy service now automatically detects and prioritizes hooks that require user decisions based on JSON decision control fields:

1. **Notification hooks** - Always require user attention (permission requests, waiting for input)
2. **PreToolUse hooks** - Require decisions when they contain permission decision fields
3. **PostToolUse hooks** - Require decisions when they contain blocking decisions
4. **UserPromptSubmit hooks** - Require decisions when they block prompts
5. **Stop/SubagentStop hooks** - Require decisions when they block stopping

## Test Scenarios

### Test 1: Notification Hook (Always High Priority)

```bash
# Simulate a notification hook that always requires user attention
echo '{
  "hook_event_name": "Notification", 
  "message": "Claude needs your permission to use Bash"
}' | CLAUDE_HOOK=Notification ./target/debug/claude-ntfy --dry-run
```

**Expected**: Priority 5, topic "claude-decisions", bypasses all filters

### Test 2: PreToolUse Hook with Permission Decision (Dynamic Priority)

```bash
# Simulate PreToolUse with permission decision field (requires decision)
echo '{
  "hook_event_name": "PreToolUse",
  "tool_name": "Bash", 
  "tool_input": {
    "command": "rm -rf /important/files"
  },
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "ask",
    "permissionDecisionReason": "Dangerous command requires confirmation"
  }
}' | CLAUDE_HOOK=PreToolUse ./target/debug/claude-ntfy --dry-run
```

**Expected**: Priority 5 (elevated from base priority 4), topic "claude-tools", bypasses filters

### Test 3: PreToolUse Hook with Deprecated Decision Field (Dynamic Priority)

```bash
# Simulate PreToolUse with deprecated decision field (requires decision)
echo '{
  "hook_event_name": "PreToolUse",
  "tool_name": "Write",
  "tool_input": {
    "file_path": "/important/config.txt"
  },
  "decision": "block",
  "reason": "Writing to important configuration file blocked"
}' | CLAUDE_HOOK=PreToolUse ./target/debug/claude-ntfy --dry-run
```

**Expected**: Priority 5 (elevated from base priority 4), topic "claude-tools", bypasses filters

### Test 4: PreToolUse Hook without Decision Fields (Normal Priority)

```bash
# Simulate PreToolUse for Read command (no decision fields)
echo '{
  "hook_event_name": "PreToolUse",
  "tool_name": "Read",
  "tool_input": {
    "file_path": "/tmp/safe-file.txt"
  }
}' | CLAUDE_HOOK=PreToolUse ./target/debug/claude-ntfy --dry-run
```

**Expected**: Priority 4 (base priority), topic "claude-tools", normal filtering

### Test 5: UserPromptSubmit with Blocking Decision (Dynamic Priority)

```bash
# Simulate UserPromptSubmit with blocking decision field
echo '{
  "hook_event_name": "UserPromptSubmit",
  "prompt": "Please help me set my password to something secure",
  "decision": "block",
  "reason": "Security policy violation: Prompt contains potential secrets"
}' | CLAUDE_HOOK=UserPromptSubmit ./target/debug/claude-ntfy --dry-run
```

**Expected**: Priority 5 (elevated from base priority 4), topic "claude-prompts", bypasses filters

### Test 6: UserPromptSubmit without Decision Fields (Normal Priority) 

```bash
# Simulate UserPromptSubmit with normal content (no decision fields)
echo '{
  "hook_event_name": "UserPromptSubmit",
  "prompt": "Write a Python function to calculate factorial"
}' | CLAUDE_HOOK=UserPromptSubmit ./target/debug/claude-ntfy --dry-run
```

**Expected**: Priority 4 (base priority), topic "claude-prompts", normal filtering

### Test 7: PostToolUse Hook with Blocking Decision (Dynamic Priority)

```bash
# Simulate PostToolUse hook with blocking decision
echo '{
  "hook_event_name": "PostToolUse",
  "tool_name": "Write",
  "tool_response": {"success": true},
  "decision": "block",
  "reason": "File format validation failed"
}' | CLAUDE_HOOK=PostToolUse ./target/debug/claude-ntfy --dry-run
```

**Expected**: Priority 5 (elevated from base priority 3), topic "claude-code-hooks", bypasses filters

### Test 8: PostToolUse Hook without Decision Fields (Normal Priority)

```bash
# Simulate PostToolUse hook (no decision fields)
echo '{
  "hook_event_name": "PostToolUse",
  "tool_name": "Write",
  "tool_response": {"success": true}
}' | CLAUDE_HOOK=PostToolUse ./target/debug/claude-ntfy --dry-run
```

**Expected**: Priority 3 (base priority), topic "claude-code-hooks", normal filtering

### Test 9: Stop Hook with Blocking Decision (Dynamic Priority)

```bash
# Simulate Stop hook with blocking decision
echo '{
  "hook_event_name": "Stop",
  "decision": "block",
  "reason": "Must run additional validation before stopping"
}' | CLAUDE_HOOK=Stop ./target/debug/claude-ntfy --dry-run
```

**Expected**: Priority 5 (elevated from base priority 2), topic "claude-sessions", bypasses filters

### Test 10: SubagentStop Hook with Blocking Decision (Dynamic Priority)

```bash
# Simulate SubagentStop hook with blocking decision
echo '{
  "hook_event_name": "SubagentStop",
  "decision": "block", 
  "reason": "Subagent must continue with additional tasks"
}' | CLAUDE_HOOK=SubagentStop ./target/debug/claude-ntfy --dry-run
```

**Expected**: Priority 5 (elevated from base priority 2), topic "claude-sessions", bypasses filters

## Configuration Commands

### Check Current Decision Hook Settings

```bash
# View decision hook priority setting
./target/debug/claude-ntfy config get hooks.decision_hook_priority

# View filter bypass setting  
./target/debug/claude-ntfy config get hooks.never_filter_decision_hooks
```

### Configure Decision Hook Behavior

```bash
# Set maximum priority for decision hooks
./target/debug/claude-ntfy config set hooks.decision_hook_priority 5

# Enable filter bypass for decision hooks
./target/debug/claude-ntfy config set hooks.never_filter_decision_hooks true

# Disable filter bypass (not recommended)
./target/debug/claude-ntfy config set hooks.never_filter_decision_hooks false
```

## Expected Behavior

When decision-requiring hooks are detected:

1. **Priority Elevation**: Automatically receive `decision_hook_priority` (default: 5)
2. **Filter Bypass**: Skip all configured filters if `never_filter_decision_hooks = true`
3. **Topic Routing**: Sent to dedicated topics for better organization
4. **Guaranteed Delivery**: Ensures important user decisions are never missed

## Real-World Use Cases

### 1. Permission Requests
When hook scripts return JSON with permission decision fields (like `permissionDecision: "ask"`), you get immediate high-priority notifications.

### 2. Blocking Decisions
When hook scripts return JSON with blocking decisions (like `decision: "block"`), they're automatically prioritized for immediate attention.

### 3. User Input Required
When Claude sends notification hooks (always decision-requiring), these notifications bypass any filters you may have set.

### 4. Hook Script Decision Control
Hook scripts can now use proper JSON output to control Claude Code behavior:
- Return `{"hookSpecificOutput": {"permissionDecision": "deny"}}` to block PreToolUse actions
- Return `{"decision": "block", "reason": "Validation failed"}` to prevent PostToolUse continuation
- Return `{"decision": "block"}` to prevent UserPromptSubmit processing or Stop/SubagentStop actions

This ensures that hooks requiring your immediate attention are never filtered out or deprioritized, while enabling proper decision control through structured JSON output rather than content analysis.