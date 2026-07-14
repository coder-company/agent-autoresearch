# Autoresearch for Pi

Pi uses a Git-installable package that provides the Autoresearch skill. The
package supplies the workflow; the native `autoresearch` binary performs
mechanical verification, rollback, and run-state management.

## Install

Install the binary first, then add the versioned Pi package:

```bash
cargo binstall autoresearch
pi install git:github.com/coder-company/agent-autoresearch@v0.1.1
```

The command records the package in Pi's user settings. To install it only for
the current project, use Pi's local settings instead:

```bash
pi install -l git:github.com/coder-company/agent-autoresearch@v0.1.1
```

Verify both parts of the installation:

```bash
autoresearch --help
pi list
```

`pi list` should include the Git source. Pi loads the `autoresearch` skill from
the package on its next resource discovery cycle.

## Start a Run

Open Pi in the target repository and describe a measurable goal. Pi can select
the skill automatically, or you can force it explicitly:

```text
/skill:autoresearch loop
Goal: Reduce TypeScript `any` occurrences to zero.
Scope: src/**/*.ts
Verify: rg -n ': any' src | wc -l
```

The skill inspects the repository and proposes a goal, scope, metric,
direction, verification command, iteration cap, and optional guard. Confirm
that plan before it starts changing files.

## Pi Execution Model

Pi-driven runs stay in the active Pi session. Do not use
`autoresearch runtime run`: that supervisor launches Codex workers rather than
Pi sessions.

For every approved iteration, Autoresearch:

1. Reads recent Git history and `autoresearch-results/` state.
2. Makes one focused change within the confirmed scope.
3. Creates a scoped trial commit.
4. Runs mechanical verification and the optional guard.
5. Keeps an improvement or discards it through the native binary.

The skill never pushes, publishes, or deploys without explicit approval. It
also keeps `autoresearch-results/` and `.codex-autoresearch/` out of trial
commits.

## Update

Git package references are pinned. To move to a later release, install the new
tag explicitly:

```bash
pi install git:github.com/coder-company/agent-autoresearch@vNEXT
```

Replace `vNEXT` with the desired release tag. Review the package source before
installing it: Pi packages run with the permissions of your local user.
