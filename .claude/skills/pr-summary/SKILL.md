---
name: pr-summary
description: Use when the user asks for a plain-language summary of what a pull request or branch changes and why it matters.
---

Produce a concise, reviewer-friendly summary of a set of changes.

1. Gather the diff: `git diff main...HEAD` (or the base the user names).
2. Identify the **intent** — what problem the change solves — from commit
   messages and the code itself, not just the mechanical diff.
3. Write the summary with these sections:
   - **What changed** — 2–4 bullets, the substantive changes.
   - **Why** — one or two sentences on the motivation.
   - **Risk / things to check** — anything a reviewer should look at closely
     (migrations, public API changes, perf-sensitive paths). Omit if none.

Keep it short. A reviewer should grasp the PR in under 30 seconds. Do not
restate every file; group related edits.
