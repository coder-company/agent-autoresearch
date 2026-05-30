use super::{AgentAdapter, InputMethod};

/// Claude Code plugin adapter.
///
/// Commands are invoked as `/autoresearch` and `/autoresearch:<subcommand>`.
/// Interactive setup uses `AskUserQuestion` (single batched call).
/// Hook system uses JSON stdin/stdout protocol.
pub struct ClaudeAdapter;

impl AgentAdapter for ClaudeAdapter {
    fn format_output(&self, content: &str) -> String {
        // Claude Code expects plain markdown output
        content.to_string()
    }

    fn command_prefix(&self) -> &str {
        "/autoresearch"
    }

    fn input_method(&self) -> InputMethod {
        InputMethod::AskUserQuestion
    }
}

impl ClaudeAdapter {
    /// Generate the hooks.json for Claude Code plugin distribution.
    pub fn hooks_json() -> serde_json::Value {
        serde_json::json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Write|Edit",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "autoresearch hook scout-block",
                                "timeout": 5
                            },
                            {
                                "type": "command",
                                "command": "autoresearch hook privacy-block",
                                "timeout": 5
                            }
                        ]
                    },
                    {
                        "matcher": "Bash",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "autoresearch hook scout-block",
                                "timeout": 5
                            },
                            {
                                "type": "command",
                                "command": "autoresearch hook privacy-block",
                                "timeout": 5
                            },
                            {
                                "type": "command",
                                "command": "autoresearch hook dangerous-cmd-block",
                                "timeout": 5
                            }
                        ]
                    },
                    {
                        "matcher": "Glob|Grep|Read",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "autoresearch hook scout-block",
                                "timeout": 5
                            },
                            {
                                "type": "command",
                                "command": "autoresearch hook privacy-block",
                                "timeout": 5
                            }
                        ]
                    }
                ],
                "UserPromptSubmit": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "autoresearch hook iteration-context",
                                "timeout": 5
                            },
                            {
                                "type": "command",
                                "command": "autoresearch hook dev-rules-reminder",
                                "timeout": 5
                            },
                            {
                                "type": "command",
                                "command": "autoresearch hook simplify-gate",
                                "timeout": 5
                            }
                        ]
                    }
                ],
                "SubagentStart": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "autoresearch hook subagent-context",
                                "timeout": 5
                            }
                        ]
                    }
                ],
                "SessionStart": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "autoresearch hook session-init",
                                "timeout": 5
                            }
                        ]
                    }
                ],
                "SessionEnd": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "autoresearch hook session-end",
                                "timeout": 5
                            }
                        ]
                    }
                ]
            }
        })
    }

    /// Generate the plugin.json manifest.
    pub fn plugin_json() -> serde_json::Value {
        serde_json::json!({
            "name": "autoresearch",
            "description": "Autonomous improvement engine. Modify → Verify → Keep/Discard → Repeat.",
            "version": env!("CARGO_PKG_VERSION"),
            "author": {
                "name": "Coder Company",
                "url": "https://github.com/coder-company"
            },
            "repository": "https://github.com/coder-company/agent-autoresearch",
            "license": "MIT",
            "keywords": [
                "autonomous",
                "iteration",
                "optimization",
                "debugging",
                "security-audit"
            ]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ClaudeAdapter;

    #[test]
    fn hooks_json_wires_dev_rules_reminder() {
        let config = ClaudeAdapter::hooks_json();
        let hooks = config
            .pointer("/hooks/UserPromptSubmit/0/hooks")
            .and_then(|value| value.as_array())
            .unwrap();
        let commands = hooks
            .iter()
            .filter_map(|hook| hook.get("command").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(
            commands,
            [
                "autoresearch hook iteration-context",
                "autoresearch hook dev-rules-reminder",
                "autoresearch hook simplify-gate",
            ]
        );
    }
}
