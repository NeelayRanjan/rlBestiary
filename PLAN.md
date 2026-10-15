# Build plan

Current task: M2 — genome encoding, bounded expansion, brains, mutation/crossover and property tests.

## M0 Scaffold
- [x] Read AGENTS.md and SPEC.md in full; fetch and check out upstream.
- [x] Create all seven workspace crates and preserve simulation/viewer boundary.
- [x] Pin direct dependencies and toolchain; add Makefile and gitignore.
- [x] Record decisions and full milestone checklist.
- [x] Generate and commit Cargo.lock.
- [x] Pass fmt, clippy, workspace tests, release build; commit M0.

## M1 Physics core and performance gate
- [x] Seeded tuple RNG and fixed-step plumbing (physics 1/240 second).
- [x] Rapier integration, articulated rigid bodies and unmotorized joints.
- [x] Contractile muscles: length-tension, area, passive stiffness/damping, work and isometric cost.
- [x] Hand-built body walks using muscle forces alone.
- [x] Mechanical energy ledger includes passive spring energy; exploit audit and tests.
- [x] Repeat identical trials with bitwise deterministic trajectories.
- [x] Criterion throughput gate: 12 parts, nine trial workloads, under two core seconds; report creature seconds per second.
- [x] Record measured benchmark, assumptions, and any gate-preserving adjustments.
- [x] Pass make check and commit M1 before M2.

## M2 Genome
- [ ] Serializable morphology graph: boxes/capsules, joints, muscle endpoints, sensors, limits, colors.
- [ ] Bounded recursive/cyclic expansion: 32 parts, depth 12, mass and minimum size.
- [ ] Reflection, subtree scale, terminal-only edges, stable muscle endpoint resolution.
- [ ] Genesis guarantee repairs missing muscles on each actuated axis.
- [ ] Distributed motor neurons, local/global sensors, oscillators, activations and Hebbian plasticity.
- [ ] Ethology recurrent brain, life-history/social genes, evolvable clamped mutation rates.
- [ ] Parameter jitter; add/remove bone/joint; change joint type/recursion; reflection; duplicate/prune subtree.
- [ ] Add/remove muscle, move attachments, change cross-section/span; add/remove sensors and neurons.
- [ ] Weight jitter, activation changes, plasticity toggle, life-history mutation.
- [ ] Valid subtree crossover with inherited muscles/neurons and joint repair.
- [ ] Symmetric compatibility distance, zero only for identical genomes.
- [ ] Binary round-trip, base64url share strings, canonical content hashes, mutation records.
- [ ] Proptest mutation sequences, crossover, bounds, axis coverage and mirrored limbs.
- [ ] Pass make check and commit M2.

## M3 Combine harness
- [ ] Stand with two-second gate and isometric cost; sprint and acceleration/cost.
- [ ] Turn, rough terrain, slope, swimming, strike, carry, reach/bulk/gait-frequency trials.
- [ ] Extract trait vector, viability, work and isometric costs.
- [ ] Cache by content hash; deterministic seeded trials; serial/parallel exact equivalence.
- [ ] Compressed sprint/strike replays, playback and trajectory tests.
- [ ] Free-energy exploit flagged, logged, and excluded; normal body accepted.
- [ ] Stable preset measurements; public harness API for future comparative arena.
- [ ] Pass make check and commit M3.

## M4 World core
- [ ] World configuration in documented config/world.ron; negative schema tests.
- [ ] Procedural terrain, temperature/moisture, seasons and solar input.
- [ ] Evolving plant biomass, toughness, toxicity, dispersal and regrowth.
- [ ] Closed nutrient/energy ledgers, feeding, carcass decay, metabolic losses.
- [ ] Trait-based movement and recurrent ethology choices; growth, senescence and death.
- [ ] Deterministic collections, subsystem/entity/tick RNG, libm, canonical full-state hash.
- [ ] World rule trait for phase-two extension; document two-brain compromise.
- [ ] Conservation for 1M ticks, five-seed population stability, hashes at 1k/100k/1M.
- [ ] World throughput >=5,000 creatures at 10,000 ticks/minute on one core.
- [ ] Pass make check and commit M4.

## M5 Ecology
- [ ] Strike/mass/armor/speed-based predation, injuries and scavenging.
- [ ] Evolving pathogens, immunity/resistance and transmission.
- [ ] Four pheromone channels, emission cost, diffusion/decay and sensing.
- [ ] Slow uplift, sea-level change, habitat fragmentation and shocks.
- [ ] Trophic persistence on three of five seeds over 1M ticks; plant/herbivore arms-race series.
- [ ] Pass make check and commit M5.

## M6 Reproduction, persistence and daemon
- [ ] Sexual reproduction, compatibility, mate preference, gestation, clutch and offspring size.
- [ ] Deterministic combine job scheduling and ordered birth admission.
- [ ] Persistent species clustering, minimum size/duration, naming, extinction and phylogeny.
- [ ] SQLite genome/creature/species/phylogeny/event/timeseries/archive/exploit/mutation tables.
- [ ] Every mutation records operator, parent/child, traits, viability, birth and lineage outcomes.
- [ ] Interval/shutdown snapshots and replay storage; reproducible restart.
- [ ] Typed events and subscribers for store/CLI/viewer.
- [ ] Local Unix socket / Windows named pipe, length-prefixed postcard codec and tests.
- [ ] Headless daemon, world new/run/attach/status commands and JSON output.
- [ ] Combine/archive/phylo/replay/genome/bench CLI commands and JSON output.
- [ ] Three coexisting species and recorded geographic split at documented seed/tick.
- [ ] Pass make check and commit M6.

## M7 Archive
- [ ] Sparse MAP-Elites grid over all seven specified descriptors and deterministic elite selection.
- [ ] Coverage definition, persistence, export and optional reseeding (off by default).
- [ ] Every archived genome reproduces recorded traits; exploit exclusion.
- [ ] >=15% reachable-cell coverage, greater body size in parts, >=3 live species at demo seed.
- [ ] Five-seed end-of-run trait variance and coverage growth tests.
- [ ] Pass make check and commit M7.

## M8 Viewer
- [ ] Native Bevy/egui dark desktop application with procedural assets only.
- [ ] Daemon attachment and offline read-only world file; attachment leaves state hash unchanged.
- [ ] Terrain/world view, proxy LOD, full bodies, free/follow/director cameras.
- [ ] Temperature/moisture/biomass/pheromone/species/predation overlays.
- [ ] Body/joints/morphology inspector, trait radar, motor/ethology activity, health/energy/age/lineage.
- [ ] Interactive phylogeny, archive axis selection and replay hover.
- [ ] Population/biomass/species/trait/coverage charts and binning/formatting tests.
- [ ] Pause/speed, replay scrub, event highlights and auto-director.
- [ ] PNG clip export and ffmpeg sidecar command.
- [ ] Headless one-frame render test and real data in every panel.
- [ ] 60 fps with 2,000 proxies and 20 articulated creatures; record measurement.
- [ ] Pass make check and commit M8.

## M9 Designer and zoo
- [ ] 3D bone/joint/muscle/recursion/reflection/sensor/life-history editing.
- [ ] Live mass/parts/metabolic-cost/bounds validity panel; muscle activation visualization.
- [ ] Combine, traits and replay from editor; founding-population release at chosen location.
- [ ] Save/load/share-string round-trip.
- [ ] Eight presets: grazer, sprinter, ambush predator, scavenger, burrower, swimmer, serpent, radial drifter.
- [ ] Every preset round-trips, combines and survives first generation.
- [ ] Pass make check and commit M9.

## M10 Ship
- [ ] RUNBOOK: three-platform builds, daemon/viewer, days-long runs, phylogeny/archive, designer, clips, config, determinism caveat.
- [ ] TUNING: every knob, muscle symptoms first, sessile-sipper, monoculture, predator/prey collapse and stagnation.
- [ ] README points to RUNBOOK; final assumptions/deviations review.
- [ ] >=80% coverage for core/sim/world; all module/config negative tests.
- [ ] make longrun: five seeds, 1M ticks, conservation, determinism and stability.
- [ ] make bench meets all budgets; record machine, commands and results in RUNBOOK.
- [ ] make dist creates runnable host binaries; offline runtime verified.
- [ ] Fresh-clone make check and all required CLI commands exercised.
- [ ] Verify every SPEC completion item; clean git status, lockfile committed, outputs ignored.
- [ ] Commit M10 and tag v0.1.0 only when completion checklist is actually true.
