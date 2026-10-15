# Completion audit at the requested handoff

2026-10-14, Apple M4 Pro / arm64, 24 GiB, macOS 15.3.1, Rust 1.85.1.
The user requested a credit-limited handoff: fix glaring issues, document, commit
and push. This is an **untagged candidate**, not a completed v1. The SPEC targets
are unchanged. No additional long experiments were started after that instruction.

Each row corresponds to one line of SPEC's Completion checklist.

| Requirement | Evidence and status |
| --- | --- |
| Required documents current; PLAN all ticked | Documents exist, including TUNING's 87/87 world-field audit. **Incomplete:** PLAN correctly retains failed and unverified items. |
| Fresh-clone `make check` | **Not run.** Working-checkout `make check` passed; `final-check.log`. A fresh temporary clone remains required. |
| Every performance budget | **Pass:** `make bench`, `bench-final.log`: 0.628458 s for the full 12-part combine; 13,700.3 ticks/minute with 5,000 creatures; 128.0886 fps with 2,000 visible proxies and 20 bodies, 2880x1920. RUNBOOK records these. Earlier failures remain in `bench-first-attempt.log` and `bench-second-attempt.log`. |
| Five-seed 1M stability/conservation/determinism | `longrun.log` passed with real births/deaths, all original founders dead, and hashes at 1k/100k/1M. **Final rerun unfinished:** that run precedes M9's zoo changes; world tick and World::new were unchanged. |
| Required CLI against fresh world | **Pass:** `scripts/check-cli`, 30 commands; `cli-summary.json`, `cli-commands.log`, `cli-validation.md`. Includes live IPC, exact recombines, archive/phylogeny, mutations, both real 61-frame PNG exports and unchanged source bytes/hash. |
| Viewer attach/offline/all long-run panels | M9 GPU/IPC checks passed (`m9-designer-render.log`, `m9-designer-release.log`); packaged offline viewer passed (`dist-smoke.json`). **Incomplete:** final all-panel million-tick audit not run. `scripts/check-longrun-panels` is prepared but not executed. |
| Designer and eight presets | **Pass:** M9 committed; `m9-check.log`, `m9-designer-release.log`, `zoo-first-generation.log`. All eight first-generation tests passed again in `final-check.log`. |
| Demo species, 15% coverage and larger species | **Fail:** seed 42, 1M ticks, five living species, 15-part living species founder versus 13-part founders, only 5/128 archive cells (3.90625%). `demo-million-ticks.log`. Denominator and bins unchanged. |
| Simulation boundary/deterministic APIs | **Pass:** `python3 scripts/test-architecture.py`, `architecture.log`; offline dependency graph for all three target platforms and production source checks. Behavioral tests also passed. This does not claim those platforms were built on this host. |
| Genesis spanning and no joint motors | **Pass:** bounded mutation/crossover/axis property tests in `final-check.log`; whole-workspace joint-motor audit in `architecture.log`. Only contractile muscle forces actuate bodies. |
| Long-run mutation dataset | Recorded fresh-world dataset audited: 1,113 mutation rows, zero missing required fields; `demo-database-audit.json`. Never-born lifespan is not applicable, with born descendants accounted for separately. This dataset precedes M9's zoo-only changes. |
| Runnable host distribution and platform build docs | **Pass:** actual `make dist`, `dist-build.log`, `dist-smoke.json`: packaged CLI creates a fresh seed-17 world, packaged daemon reaches tick 10, packaged viewer renders it offline without changing state, all checksums verify. RUNBOOK contains macOS/Linux/Windows build commands. |
| Clean status, lockfile, ignored outputs and v0.1.0 tag | Lockfile tracked and generated directories ignored. Final commit/push is the handoff step. **No tag:** numeric failures and unfinished validation prohibit `v0.1.0`. |
| RUNBOOK topics | **Pass:** build, create/run, attach, days-long operation, archive/phylogeny, Designer/release, clips, tuning and cross-platform replay caveat documented. |

Additional binding acceptance still unmet:

- M5 trophic persistence: recorded 0/5, requires 3/5 for 1M ticks. Plant/herbivore
  arms-race series failed; herbivores collapsed by 800k. Four tuning candidates
  and full failed runs are documented in ASSUMPTIONS and the ecology logs. The
  trophic fixture uses zoo genomes, so rerun it after M9 before claiming current
  preset results.
- M7 stagnation guard: all five coverage curves plateaued in the final quarter;
  trait variance increased only for seed 3. `evolution-million-ticks.log`.
- M10 >=80% executable-line coverage per core/sim/world crate: **unmeasured**.
  `make coverage` and six accounting regressions are implemented; LLVM tooling is
  installed. No coverage percentage is claimed.
- Fresh-clone check, final current-binary long-run revalidation and all long-run
  viewer panels remain for the next session. Do not tag until every item passes.

The distribution is local under ignored `dist/`; databases and PNG evidence are
also intentionally ignored. Committed reports retain commands, paths and hashes.
