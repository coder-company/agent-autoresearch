# Release Checklist

Manual steps for cutting a release. For automation, use `./scripts/release.sh <version>`.

## Pre-release

- [ ] All tests pass: `cargo test`
- [ ] Zero clippy warnings: `cargo clippy -- -D warnings`
- [ ] Code formatted: `cargo fmt -- --check`
- [ ] E2E fixtures still valid against current schemas
- [ ] `COMPARISON.md` is current (if upstream projects changed)
- [ ] `README.md` reflects any new commands or features

## Version Bump

- [ ] Update `version` in `Cargo.toml`
- [ ] Update `docs/changelog.md` with actual changes
- [ ] Update `.claude-plugin/plugin.json`
- [ ] Update `plugins/autoresearch/.codex-plugin/plugin.json` (`<VERSION>-codex.0`)
- [ ] Update skill frontmatter in `skills/autoresearch/SKILL.md` and `.agents/skills/autoresearch/SKILL.md`
- [ ] Run `./scripts/transform.sh` so `.opencode/` and `plugins/autoresearch/skills/` inherit the version bump
- [ ] Run `cargo build --release` to regenerate `Cargo.lock`

## Build & Verify

- [ ] `cargo build --release` succeeds
- [ ] Binary size is reasonable (~2.5MB): `du -h target/release/autoresearch`
- [ ] Smoke test: `./target/release/autoresearch --version`
- [ ] Smoke test: `./target/release/autoresearch status --cwd /tmp` (should error cleanly)

## Tag & Push

- [ ] `git add Cargo.toml Cargo.lock docs/changelog.md .claude-plugin/plugin.json skills/autoresearch/SKILL.md .opencode/skills/autoresearch/SKILL.md .agents/skills/autoresearch/SKILL.md plugins/autoresearch`
- [ ] `git commit -m "release: v<VERSION>"`
- [ ] `git tag -a v<VERSION> -m "Release v<VERSION>"`
- [ ] `git push origin main --tags`

## GitHub Release

- [ ] `gh release create v<VERSION> --generate-notes`
- [ ] Upload binary: `gh release upload v<VERSION> target/release/autoresearch`
- [ ] Review auto-generated notes, edit if needed

## Post-release

- [ ] Verify install script works: `curl -sL <url> | sh`
- [ ] Run `./scripts/transform.sh` to update distributions
- [ ] Announce if applicable
