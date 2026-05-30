# Core Principles

## 1. Constraint Enables Autonomy
Bounded file set. Single metric. Fixed iteration cost.

## 2. Humans Set Direction, Agents Execute
User defines the goal. Agent decides how to test ideas within boundaries.

## 3. Metrics Must Be Mechanical
If a command cannot decide whether the result improved, the loop is not ready.

Good: test count, coverage %, bundle size, latency, error count.
Bad: "looks better", "feels cleaner", "probably faster".

## 4. Fast Verification Wins
Targeted tests over full suites. Incremental builds over full rebuilds.

## 5. One Change Per Iteration
Atomic experiments create causality. If the result changes, you know why.

## 6. Git Is Memory
Kept experiments stay in history. Failed experiments revert. Results TSV is the audit trail.

## 7. Simplicity Is A Tiebreaker
Equal metric + less complexity = keep. Marginal gain (<1%) + added complexity = discard.

## 8. Honest Limits
If permissions, tooling, or context make the loop unsafe — stop and say so.
