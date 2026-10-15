# Assumptions and decisions

- The existing local repository had no commits or files; it now uses upstream main at e77d504 as its starting point.
- Rust is installed under ignored `.tools/` because this machine has no Rust installation. `scripts/cargo` selects that toolchain or falls back to the user's Cargo.
- Direct dependency versions are exact and the lockfile pins transitive dependencies; dependencies enter each crate when first needed.
- M0 binary entry points explicitly report that implementation is pending and exit unsuccessfully; they are scaffolding, not completed features.

## Deviations

The M1 performance gate is staged because the named combine trials, world and viewer are assigned to M3, M4 and M8 by the specification. M1 gates measured representative physics throughput; the later budgets remain binding before release.

- M1's throughput benchmark runs nine two-second physics workloads on the 12-part fixture, including actuation, energy accounting and complete trajectories. M3 replaces this representative workload with the nine named combine trials; final release must meet the full combine budget, world budget and renderer budget independently.
- World and viewer budgets cannot be measured in M1 before those systems exist; their measured gates remain mandatory in M4/M8 and M10. No proxy result is reported as evidence that those budgets pass.
- M1 uses SI units, a fixed 1/240-second tick and configurable internal substeps; physical constants and numerical audit tolerances are stored in config/world.ron.

- Genome muscle material genes scale the documented world baselines for contraction range, passive stiffness and damping, so both inherited variation and user tuning affect physics.
- Expansion uses branch-local recursion counts and deterministic depth-first order; a bound truncates that branch without invalidating the remaining body. Routes clipped by expansion bounds are omitted and missing leverage is repaired deterministically.
- Compatibility includes normalized part/joint-count, species-marker and measured-trait distances, with an identity-separation term for otherwise neutral alleles.
- Mutation records include no-op proposals (for example removal at a minimum bound); downstream evaluation records the actual child hash.
