# Prompts

Two prompts. Paste the first one to start the build. Paste the second only if the
agent comes back in a fresh session after a usage reset.

## Start

```
Build bestiary, the project specified in this repo. Read AGENTS.md and SPEC.md in
full before doing anything else, then build it end to end without asking me a
single question.

AGENTS.md is binding: no questions, record every decision in ASSUMPTIONS.md, one
milestone at a time with a commit at the end of each, tests with every feature,
nothing at runtime that needs the network or a downloaded asset. The determinism
rules in SPEC.md are correctness requirements, not preferences.

M1 is a gate. Build the physics core and the throughput benchmark before anything
that sits on top of them, and do not move to M2 until the benchmark meets the
performance budget. If it can't, change the trial design or the world tiering
until it does and record what you changed.

Start at M0 by writing PLAN.md with a checkbox for every task in every milestone,
then work straight through M0 to M10. Keep PLAN.md ticked as you go so a fresh
session can resume from it. Do not stop, do not report back for approval, and do
not declare the build finished until every line of the Completion checklist in
SPEC.md is true.
```

## Resume

```
You are resuming an unattended build of bestiary. Do not ask me anything.

1. Read AGENTS.md, SPEC.md, PLAN.md, and ASSUMPTIONS.md in full, then run
   `git log --oneline` and `git status`.
2. If the working tree is dirty, finish or revert the in-progress change so
   `make check` passes, then commit.
3. Find the first unticked task in PLAN.md and continue from there, following
   every rule in AGENTS.md.
4. Keep going, milestone by milestone, until every item in the Completion
   checklist in SPEC.md is true. Then tag v0.1.0.
```
