#!/usr/bin/env bash
# Score a codebase: debug (find issues) → fix (resolve them)
# Usage: scripts/score-debug-fix.sh [--scope "src/**/*.ts"] [--iterations 15]
set -euo pipefail

SCOPE=""
ITERATIONS=15
EXTRA_ARGS=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --scope) SCOPE="$2"; shift 2 ;;
    --iterations) ITERATIONS="$2"; shift 2 ;;
    *) EXTRA_ARGS="$EXTRA_ARGS $1"; shift ;;
  esac
done

TIMESTAMP=$(date +%Y%m%d-%H%M%S)
LOG_DIR="autoresearch-results/score-${TIMESTAMP}"
mkdir -p "$LOG_DIR"

echo "=== Autoresearch Score: Debug → Fix ==="
echo "Scope: ${SCOPE:-entire codebase}"
echo "Iterations per phase: $ITERATIONS"
echo "Log directory: $LOG_DIR"
echo ""

# Phase 1: Debug (find issues)
echo "--- Phase 1: Debug (finding issues) ---"
DEBUG_ARGS="Iterations: $ITERATIONS --chain fix"
[[ -n "$SCOPE" ]] && DEBUG_ARGS="Scope: $SCOPE $DEBUG_ARGS"

echo "Config: /autoresearch:debug $DEBUG_ARGS $EXTRA_ARGS"
echo "Run this command in your agent session:"
echo ""
echo "  /autoresearch:debug $DEBUG_ARGS $EXTRA_ARGS"
echo ""
echo "Results will chain automatically to fix mode."
echo "Combined results logged to: $LOG_DIR"
