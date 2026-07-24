---
name: Plan
about: The plan of record for a unit of work, before and during implementation.
labels: plan
---

<!--
This description is the LIVING PLAN — the single, current source of truth for
this work. Edit it as understanding evolves so it always reflects the present
intent; a reader should understand the plan from the description alone,
without reconstructing it from the comment thread.

Every time you edit the description, add a comment explaining what changed
and why. Comments are the audit trail; the description is the present state.
-->

## Intent

<!-- What this work delivers and why it matters, in a paragraph or two.
Describe the intended end state, not the steps taken to get here. -->

## Scenarios

<!-- What "done" means, as concrete, testable behavior. Cover the happy path,
the boundaries, and the error paths. Each scenario maps to one or more tests
written first, before the production code. -->

```gherkin
Feature: <capability>

  Scenario: <a specific, observable behavior>
    Given <starting state>
    When <action>
    Then <expected, checkable outcome>
```

## Out of scope

<!-- What this work deliberately does not cover. If something here still
needs to happen, it gets its own issue. -->

## Open questions

<!-- Unresolved decisions blocking or shaping the work. Resolve them in
comments, then fold the answers back into the sections above and delete this
section when it empties. -->
