---
description: Draft a changelog entry from the current branch's commits since main.
---

Summarize the changes on the current branch into a user-facing changelog entry.

Steps:

1. Run `git log --oneline main..HEAD` to see the commits ($ARGUMENTS may name a
   different base branch — use it if provided).
2. Group the changes into **Added**, **Changed**, **Fixed**, and **Removed**.
   Omit any empty group.
3. Write each entry from the *user's* perspective — what changed for them, not
   the internal implementation detail.

Output only the changelog markdown, ready to paste under a version heading.
