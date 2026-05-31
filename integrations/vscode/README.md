# Autoresearch VS Code Extension

This lightweight extension delegates to the installed `autoresearch` binary.

Commands:

- `Autoresearch: Show Status` runs `autoresearch status --summary`.
- `Autoresearch: Show Dashboard` runs `autoresearch dashboard --once`.
- `Autoresearch: Watch Results` opens a terminal with `autoresearch watch --format jsonl`.

Set `autoresearch.binaryPath` when the binary is not on `PATH`.
