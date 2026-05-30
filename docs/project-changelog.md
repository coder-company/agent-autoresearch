# Project Changelog

This page is the high-level release history entrypoint. The canonical
Keep-a-Changelog file is [changelog.md](changelog.md).

## Current Development Track

Recent work has focused on catching the binary and installable agent packages up
to the stronger autoresearch implementations:

- Background runtime control through `autoresearch runtime run` and
  `runtime start/status/supervise/stop`
- Live log monitoring through `autoresearch watch`
- Native parallel worker support through `autoresearch parallel prepare`, `run`,
  verified `closeout`, and `cleanup`, including worker crash/timeout recording
- Codex, Claude Code, and OpenCode installation paths
- Distribution validation for generated command and skill packages
- Binary smoke tests for installed skill instructions
- Direct documentation entrypoints for installation, usage, examples, and
  system architecture

## Release Notes

See [changelog.md](changelog.md) for versioned release notes and
[development-roadmap.md](development-roadmap.md) for planned work.
