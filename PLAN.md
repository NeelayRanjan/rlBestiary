# Build plan

Current task: M9 — Designer and zoo; M7 runs continue in parallel, and their numeric results will be revisited after implementation at the user’s request.

## MVP checkpoint (scope changed at the user's request)
- [x] Freeze scope at the working simulation, native viewer, starter zoo and recorded export.
- [x] Keep determinism, conservation, exploit detection and normal regression tests binding.
- [x] Add short build/run instructions and accurately label unfinished v1 work.
- [x] Verify one-command demo startup/shutdown and actual GPU rendering/export, including actual saved-world inspection and four launcher lifecycle regressions.
- [x] Record current throughput gates after the final movement changes: combine 0.755 s, world 12,767 ticks/minute, viewer 116.47 fps.
- [x] Complete final checks and make a local, pushable MVP commit; no full v1 tag.

The MVP scope freeze was lifted at the user’s request on 2026-10-14. Resume M5/M7 acceptance, M9 and M10 in that order; the full SPEC Completion checklist is binding. Numeric failures remain unticked and prohibit tagging.

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
- [x] Serializable morphology graph: boxes/capsules, joints, muscle endpoints, sensors, limits, colors.
- [x] Bounded recursive/cyclic expansion: 32 parts, depth 12, mass and minimum size.
- [x] Reflection, subtree scale, terminal-only edges, stable muscle endpoint resolution.
- [x] Genesis guarantee repairs missing muscles on each actuated axis.
- [x] Distributed motor neurons, local/global sensors, oscillators, activations and Hebbian plasticity.
- [x] Ethology recurrent brain, life-history/social genes, evolvable clamped mutation rates.
- [x] Parameter jitter; add/remove bone/joint; change joint type/recursion; reflection; duplicate/prune subtree.
- [x] Add/remove muscle, move attachments, change cross-section/span; add/remove sensors and neurons.
- [x] Weight jitter, activation changes, plasticity toggle, life-history mutation.
- [x] Valid subtree crossover with inherited muscles/neurons and joint repair.
- [x] Symmetric compatibility distance, zero only for identical genomes.
- [x] Binary round-trip, base64url share strings, canonical content hashes, mutation records.
- [x] Proptest mutation sequences, crossover, bounds, axis coverage and mirrored limbs.
- [x] Pass make check and commit M2.

## M3 Combine harness
- [x] Stand with two-second gate and isometric cost; sprint and acceleration/cost.
- [x] Turn, rough terrain, slope, swimming, strike, carry, reach/bulk/gait-frequency trials.
- [x] Extract trait vector, viability, work and isometric costs.
- [x] Cache by content hash; deterministic seeded trials; serial/parallel exact equivalence.
- [x] Compressed sprint/strike replays, playback and trajectory tests.
- [x] Free-energy exploit flagged, logged, and excluded; normal body accepted.
- [x] Stable preset measurements; public harness API for future comparative arena.
- [x] Pass make check and commit M3.

## M4 World core
- [x] World configuration in documented config/world.ron; negative schema tests.
- [x] Procedural terrain, temperature/moisture, seasons and solar input.
- [x] Evolving plant biomass, toughness, toxicity, dispersal and regrowth.
- [x] Closed nutrient/energy ledgers, feeding, carcass decay, metabolic losses.
- [x] Trait-based movement and recurrent ethology choices; growth, senescence and death.
- [x] Deterministic collections, subsystem/entity/tick RNG, libm, canonical full-state hash.
- [x] World rule trait for phase-two extension; document two-brain compromise.
- [x] Revalidate conservation for 1M ticks, five-seed stability and hashes at 1k/100k/1M with actual births and deaths on the current binary (evidence/v1/longrun.log).
- [x] World throughput >=5,000 creatures at 10,000 ticks/minute on one core.
- [x] Pass make check and commit M4.

## M5 Ecology
- [x] Strike/mass/armor/speed-based predation, injuries and scavenging.
- [x] Evolving pathogens, immunity/resistance and transmission.
- [x] Four pheromone channels, emission cost, diffusion/decay and sensing.
- [x] Slow uplift, sea-level change, habitat fragmentation and shocks.
- [ ] Trophic persistence on three of five seeds over 1M ticks; plant/herbivore arms-race series. Fresh full runs failed (0/5; herbivore collapse by 800k), evidence/v1/ecology-longrun.log.
- [x] Add named acceptance tests and run four tuning candidates; preserve failed gates under the user’s numeric-failure exception.
- [x] Pass make check and commit M5.

## M6 Reproduction, persistence and daemon
- [x] Sexual reproduction, compatibility, mate preference, gestation, clutch and offspring size.
- [x] Deterministic combine job scheduling and ordered birth admission.
- [x] Persistent species clustering, minimum size/duration, naming, extinction and phylogeny.
- [x] SQLite genome/creature/species/phylogeny/event/timeseries/archive/exploit/mutation tables.
- [x] Every mutation records operator, parent/child, traits, viability, birth and lineage outcomes.
- [x] Interval/shutdown snapshots and replay storage; reproducible restart.
- [x] Typed events and subscribers for store/CLI/viewer.
- [x] Local Unix socket / Windows named pipe, length-prefixed postcard codec and tests.
- [x] Headless daemon, world new/run/attach/status commands and JSON output.
- [x] Combine/archive/phylo/replay/genome/bench CLI commands and JSON output.
- [x] Three coexisting species and recorded geographic split at documented seed/tick.
- [x] Pass make check and commit M6.

## M7 Archive
- [x] Sparse MAP-Elites grid over all seven specified descriptors and deterministic elite selection.
- [x] Coverage definition, persistence, export and optional reseeding (off by default).
- [x] Every archived genome reproduces recorded traits; exploit exclusion.
- [ ] >=15% reachable-cell coverage, greater body size in parts, >=3 live species at demo seed.
- [ ] Five-seed end-of-run trait variance and coverage growth tests.
- [x] Pass make check and commit M7 mechanics; long-run acceptance remains explicitly open.

## M8 Viewer
- [x] Coordinated inherited muscle gait with stance/swing contact, measured support/slip/clearance and deterministic replay tests.
- [x] Inspector preserves absolute physical foot positions; tracking camera, aligned ground, visible contact shapes and fixed grid.
- [x] Replace emissive pink muscles with restrained thinner tissue, compatible body colors and material tests.
- [x] Validate locomotion, genome/physics regressions, combine/viewer throughput and make check; capture walking demo (87.2% less stance slip; 112.05 fps).
- [x] Research Evolution's official muscle, controller, morphology and cosmetic-skin documentation.
- [x] Procedural bone/joint/tendon/muscle anatomy, live activation, recorded-pose motion and collision view.
- [x] Anatomy geometry/config/attachment regressions, GPU comparison and performance gate; make check (105 fps, Apple M4 Pro, 2,000 proxies + 20 full bodies).
- [x] Regression: initial inspector body matches the selected living creature; historical genomes are labeled separately.
- [x] Native Bevy/egui dark desktop application with procedural assets only.
- [x] Daemon attachment and offline read-only world file; attachment leaves state hash unchanged.
- [x] Terrain/world view, proxy LOD, full bodies, free/follow/director cameras.
- [x] Temperature/moisture/biomass/pheromone/species/predation overlays.
- [x] Body/joints/morphology inspector, trait radar, motor/ethology activity, health/energy/age/lineage.
- [x] Interactive phylogeny, archive axis selection and replay hover.
- [x] Population/biomass/species/trait/coverage charts and binning/formatting tests.
- [x] Pause/speed, replay scrub, event highlights and auto-director.
- [x] PNG clip export and ffmpeg sidecar command.
- [x] Headless one-frame render test and real data in every panel.
- [x] 60 fps with 2,000 proxies and 20 articulated creatures; final MVP 116.47 fps with 2,020 actual agents and the full UI after virtualizing offscreen sidebar rows.
- [x] CLI replay PNG export against recorded anatomy, with complete ID and file sequences verified (61 frames each, 2880×1920, unchanged database/hash).
- [x] Conservative repeated-airborne-gait and pheromone-guided-hunt event detectors, historical predation subjects and paused-release event persistence.
- [x] Pass make check and commit M8 as the user-requested MVP checkpoint.

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
- [x] make longrun: five seeds, 1M ticks, conservation, determinism and stability; finite founders, 10,785–11,417 births per seed (evidence/v1/longrun.log).
- [ ] make bench meets all budgets; record machine, commands and results in RUNBOOK.
- [ ] make dist creates runnable host binaries; offline runtime verified.
- [ ] Fresh-clone make check and all required CLI commands exercised.
- [ ] Verify every SPEC completion item; clean git status, lockfile committed, outputs ignored.
- [ ] Commit M10 and tag v0.1.0 only when completion checklist is actually true.
