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

- M3 uses two internal physics substeps per fixed tick. The original one-substep fixture passed, but the genome-driven normal creature produced numerical energy false positives in the full harness. Two substeps resolved those without widening the energy tolerances; the deliberate overlapping-body exploit still triggers detection.
- Mass properties are recomputed before the first audited step, so attaching a carry load changes initial potential energy rather than appearing as unexplained generated energy.
- Fluid buoyancy and drag contribute explicitly measured external work to the mechanical ledger. Strike impulse excludes contacts between the target and the floor.
- Replay files use versioned lossless byte run-length compression, a bounded decoder and a content digest. Replaying uses recorded poses rather than cross-platform re-simulation.
- M3 adds the eight authored zoo fixtures for deterministic evaluation tests; their designer integration and first-generation survival checks remain in M9.
- The test profile uses optimization level 2 to keep physical property/regression suites practical; debug assertions remain enabled.

- M4 resource accounting uses integer millijoules and microkilograms so transfers conserve exactly. Nutrients eaten but not retained for growth are excreted to soil; metabolic energy leaves through the dissipated-energy ledger.
- The M4 million-tick fixture uses eight founders on an 8x8 map and a senescence age of two million ticks, because sexual reproduction is assigned to M6. This tests conservation, deterministic continuation and feeding stability before reproduction exists; it does not establish the final population-turnover acceptance result.
- The world performance fixture disables starvation damage for its short measurement window to retain exactly 5,000 agents; feeding, metabolic accounting, brains, plants and movement still run. Ecological survival is tested separately without that override.

- M5's million-tick trophic coexistence and plant/herbivore arms-race acceptance checks are deferred until reproduction is present in M6. A finite non-reproducing prey population cannot validate sustainable predator turnover. Mechanism tests and conservation checks remain required at M5, and the deferred acceptance boxes remain unticked.

- M6 mutations are evaluated synchronously in fixed proposal order, including intermediate mutants that are never born. Existing genome hashes reuse stored traits; asynchronous worker completion never controls birth order.
- Reproductive energy and nutrients are transferred from parents, with gestation loss explicitly dissipated. Designer releases draw their resources from existing soil and plants rather than creating biomass.
- Replay byte caches are derived artifacts, excluded from canonical simulation hashes and stored separately in SQLite. Snapshots retain a replay-cache field for schema compatibility; those bytes are not needed for simulation. The daemon frees successfully persisted replay caches to bound resident memory.
- `config/demo.ron` records the complete seed-42 diversification scenario; `config/world.ron` remains the baseline. Both use the same validated schema and are embedded in created worlds.
- macOS accepted local sockets are explicitly switched to blocking mode before framed I/O; otherwise large snapshot writes can return WouldBlock despite accept-only nonblocking listener configuration.

- Terrain frequency is configurable; the demo uses two waves per axis and a larger sea-level cycle to produce connected land that later fragments. Terrestrial mating requires the same connected land region, and movement integrates across cells at the measured land/swim speed.
- Senescence reduces the maximum attainable health after onset; feeding cannot heal away aging. The regression test verifies death in a fed population.
- M6's typed event stream is consumed by SQLite and exposed over IPC; archive-cell events arrive in M7 and remaining director heuristics in M8. Geographic acceptance establishes a recorded split after fragmentation, not a causal experiment proving fragmentation was the sole cause.
- M6 demo: seed 42, config/demo.ron, 24 founders, 100,000 ticks: 81 living creatures, 11 living species, first fragmentation at tick 11,000. The named acceptance test checks coexistence, descendant species after fragmentation, actual turnover, mutations and conservation.
