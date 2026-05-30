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
                "PostToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "autoresearch hook verify-capture",
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
                "Stop": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "autoresearch hook stop-check",
                                "timeout": 5
                            }
                        ]
                    }
                ],
                "PostCompact": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "autoresearch hook compaction-reanchor",
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
                "SubagentEnd": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "autoresearch hook subagent-collect",
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
                ],
                "PreCompact": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "autoresearch hook pre-compact-snapshot",
                                "timeout": 5
                            }
                        ]
                    }
                ],
                "Notification": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "autoresearch hook notification-relay",
                                "timeout": 3
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
    fn hooks_json_matches_packaged_hooks() {
        let config = ClaudeAdapter::hooks_json();
        let packaged = include_str!("../../hooks/hooks.json")
            .replace("${CLAUDE_PLUGIN_ROOT}/bin/autoresearch", "autoresearch");
        let packaged: serde_json::Value = serde_json::from_str(&packaged).unwrap();

        assert_eq!(config, packaged);
    }
}
