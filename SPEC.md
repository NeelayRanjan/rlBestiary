# bestiary — specification

Read `AGENTS.md` for the operating rules before building anything from this file.

## Mission

Build `bestiary`, a native desktop artificial-life simulator in which 3D creatures
with evolvable bodies live, reproduce, and speciate in a persistent world, end to
end, with no human in the loop. The run ends when every item in the Completion
checklist is true and `make check` passes on a fresh clone.

A creature is built from bones, joints, and contractile muscles. Its morphology is
not a fixed skeleton with tunable parameters: it is a recursive graph that can grow
new segments, new limbs, and new symmetries through mutation, so that over long
runs genuinely new body plans appear rather than better versions of the starting
plan. Bodies are evaluated in real 3D rigid-body physics. The world they live in runs on traits distilled from that physics, which
is what makes a large living ecosystem and real articulated bodies affordable at
the same time.

The user can watch the world, inspect any creature's body and brain, browse the
phylogeny, browse an archive of every body plan discovered, design a creature by
hand and release it, and leave a world running for days.

The lineage is Karl Sims' evolved virtual creatures (1994) for the encoding, and
Keiwan's Evolution for the design-and-watch loop. Prior art for the ecosystem side
is The Bibites. This is 3D, open-ended, native, and inspectable.

Runs on Linux, Windows, and macOS. No network at runtime, no accounts, no
telemetry, no downloaded assets.

## Product scope

In scope for v1:

- A recursive-graph genome covering morphology (bones, joints, muscles), motor
  control, world-level behavior, reproduction strategy, and mate preference.
- Instantiation of that genome into a 3D articulated rigid body actuated by
  contractile muscles, with a controller whose topology mirrors the body's.
- A combine harness: short, deterministic, parallel physics trials that measure
  what a body can do and cache the results as a trait vector plus a replay.
- A world tier that simulates thousands of creatures cheaply using those traits:
  terrain, climate, seasons, evolving plants, a closed nutrient loop, foraging,
  predation, injury, pathogens, pheromones, growth and senescence, sexual
  reproduction with mate choice, and slow geographic change.
- Speciation by genetic compatibility, with a persistent phylogeny.
- A MAP-Elites archive over morphological and behavioral descriptors: a browsable
  grid of every body plan discovered, each cell holding a working creature and its
  replay.
- An event bus and auto-director that detect and cut to milestones (first
  predation, first swimmer, species born, population crash).
- A native 3D viewer that attaches to and detaches from a running simulation:
  world view, creature inspector, phylogeny, archive browser, charts, replay
  scrubbing, time control.
- A creature designer: build a body by hand, test it in the combine arena, release
  it into a world.
- A headless daemon plus CLI so a world can run unattended for days.
- Persistent storage of genomes, lineage, events, and time series in a queryable
  database.

Out of scope for v1 (do not build, do not stub beyond an interface):

- Multiplayer, any network service, accounts, cloud sync, telemetry.
- GPU-accelerated physics, VR, procedural audio.
- Symbiosis, creature fusion, multicellular development from a cell, tool use,
  culture.
- Scripted scenario plugins, the experiment sweep runner, and director mode. These
  are phase two and are specified at the end of this document so the boundaries
  exist in v1.
- Per-creature gradient-based reinforcement learning. Brains are evolved. See
  "Why evolution and not RL" below.

## Why evolution and not RL

Stated here because it is the decision most likely to be second-guessed mid-build.
Gradient RL across hundreds of agents with heterogeneous morphologies, in a
non-stationary multi-agent world where death is the only episode boundary, is a
research problem. Neuroevolution needs no reward shaping, no credit assignment,
and no gradient, handles arbitrary body graphs natively, and gives fitness for
free: fitness is reproduction. In-lifetime adaptation comes from evolved Hebbian
plasticity on a subset of connections, which costs almost nothing and is visible
in the inspector. Do not introduce backpropagation anywhere in this project.

## Architecture

A Rust workspace. The simulation is a library with a headless daemon in front of
it; the viewer is a separate binary that attaches to the daemon. The simulation
crates contain no rendering, no UI, and no async.

```
bestiary-core      genome, morphology graph, brains, mutation, crossover,
                   compatibility, serialization, seeded RNG, fixed-point helpers
bestiary-sim       body instantiation, muscle actuation, physics trials,
                   metabolic model, combine harness, trait extraction, replays
bestiary-world     terrain, climate, plants, nutrient loop, agents, foraging,
                   predation, pathogens, pheromones, lifecycle, reproduction
bestiary-archive   MAP-Elites, speciation, phylogeny, SQLite store, replay store
bestiary-daemon    long-running process, job scheduler, event bus, IPC server
bestiary-viewer    Bevy app: 3D world view, inspector, phylogeny, archive, charts
bestiary-cli       command-line front end to the daemon and to offline tools
```

Key crates, pinned: `rapier3d` with the `enhanced-determinism` feature for
physics, `nalgebra`, `rand_chacha` for reproducible RNG, `serde` with `postcard`
for binary state and `ron` for human-editable config, `rusqlite` with the
`bundled` feature, `rayon` for trial-level parallelism only, `proptest` for
property tests, `criterion` for benchmarks, `bevy` plus `bevy_egui` for the viewer
only. Do not use `bevy_rapier3d`; the simulation owns Rapier directly and the
viewer only draws what the simulation reports.

IPC between daemon and viewer: a local Unix socket, or a named pipe on Windows,
carrying length-prefixed postcard frames. The viewer may also open a world file
directly in read-only mode when no daemon is running.

Make targets: `setup`, `check` (fmt, clippy, test, release build), `test`,
`bench` (the throughput gate), `run` (daemon plus viewer on a scratch world),
`dist` (release binaries for the host platform into `dist/`), `longrun` (headless
stability run used by the acceptance tests).

## Genome and encoding

One genome is a single serializable value, round-trippable to a compact binary
form and to a short base64url string for sharing. Its parts:

**Morphology graph.** A directed graph, cycles allowed, with a designated root
node.

- A node is a **bone**: a rigid segment, shape (box or capsule), dimensions,
  density, a color gene, a `recursion_limit`, and a list of sensors attached to it.
- An edge is a **joint** from parent bone to child bone: attachment point on the
  parent's surface, orientation, a scale factor applied to the child subtree, joint
  type (fixed, hinge, universal, ball), per-axis angle limits, a `reflect` flag
  that mirrors the subtree across the parent's sagittal plane, and a
  `terminal_only` flag that attaches the subtree only at the last recursion.
  Joints have no motors. Nothing moves except by muscle.
- A **muscle** is a contract-only linear actuator listed on the node that owns its
  origin: an origin point on that bone's surface, an insertion point on a bone
  reachable within `span` joints (default maximum span 2, so a muscle may cross
  more than one joint), a rest length derived at instantiation, a maximum
  contraction fraction, a cross-sectional area gene, and a passive stiffness and
  damping. Force is contraction-only, proportional to area and to the activation
  signal from the motor brain, scaled by a length-tension curve that falls off
  away from rest length. Muscles pull along the straight line between their
  endpoints; there is no routing or wrapping. Metabolic cost is the mechanical
  work actually performed plus an isometric holding cost proportional to area and
  activation.

**Genesis guarantee.** Every joint in an instantiated body is spanned by at least
one muscle per actuated axis. At genesis and after any mutation that creates a
joint, the builder inserts the missing muscles with randomized but valid
attachment points. A body is never instantiated with an unactuatable joint; the
builder asserts this and the test suite proves it. Without this rule, random
mutation produces mostly inert creatures and the population never gets started.
- A self-edge or a cycle expands by repeated instantiation up to the node's
  `recursion_limit`. This is where segmented and repeated structures come from and
  is the single most important property of the encoding. A mutation to one
  recursion count adds an entire working segment.

Expansion is bounded: maximum 32 parts, maximum depth 12, maximum total mass, and
a minimum part size. Expansion that would exceed a bound stops cleanly rather than
failing; the resulting body is still valid.

**Motor brain.** Distributed, mirroring the morphology. Each instantiated bone
carries a small pool of neurons wired to that bone's own sensors and to the
muscles originating on it, plus connections to the parent bone's pool and its
children's pools, plus a few global inputs. When a subtree is duplicated or
reflected, its neurons, muscles, and wiring come with it, already functional. This
is what stops structural mutations from being automatically lethal.

- Local sensors per bone: joint angle and angular velocity per axis, muscle length
  and contraction velocity (proprioception), contact boolean, contact force, local
  orientation relative to gravity.
- Global inputs: a clock, evolvable sine oscillators with their own frequency and
  phase genes, body orientation, and body velocity.
- Neuron types: weighted sum with one of a small set of activations (tanh, sigmoid,
  sin, abs, threshold), plus optional per-connection Hebbian plasticity with
  evolvable learning rate and decay.
- Outputs: one activation signal in [0, 1] per muscle. There is no torque output
  and no joint-space control anywhere in the project; leverage, timing, and
  antagonist balance are things evolution has to solve rather than things the
  engine hands out.

**Ethology brain.** A small recurrent network used in the world tier, where
physics is not simulated. Inputs: own energy, age, health, size relative to
neighbors, local plant density, gradients of up to four pheromone channels,
nearest creature's distance, size and species-similarity, temperature, water
proximity, time of day and season. Outputs: a move direction and effort, plus
action preferences (forage, pursue, flee, attack, mate, rest, emit pheromone per
channel). Outputs are desires; what the creature can actually achieve is bounded
by its traits, which come from the combine. This two-brain split exists for
performance and is the central compromise of the design: keep the boundary
explicit and documented in `bestiary-world/README.md`.

**Life-history and social genes.** Diet efficiency split between plant and flesh,
which trades off against itself; offspring size and clutch size; maturation age;
senescence onset; immune investment; pheromone channel and emission strength;
mate-preference weights over observable traits (size, ornament extent, symmetry,
pheromone intensity); and a species marker vector used in compatibility.

**Mutation operators**, each with its own rate in the config file and each tagged
by name on the mutation record it produces: parameter jitter on any scalar, add or
remove bone, add or remove joint, change joint type, change recursion limit,
toggle reflection, duplicate a subtree, prune a subtree, add muscle across an
existing joint, remove muscle, move a muscle attachment point, change a muscle's
cross-section, extend or shorten a muscle's span, add or remove sensor, add or
remove neuron, jitter weights, change activation, toggle plasticity on a
connection, and jitter life-history genes. Rates are per-genome-copy and are
themselves evolvable within a clamped range. Any operator that leaves a joint
unspanned triggers the genesis guarantee rather than producing an inert body.

**Crossover.** Sexual reproduction grafts a subtree from one parent onto a
compatible attachment point of the other, carrying that subtree's muscles and
neurons, and blends life-history genes. Muscles that spanned the severed joint are
dropped and replaced by the genesis guarantee at the new attachment. Compatible means similar attachment geometry and scale.
The result must always be a valid genome; the test suite proves it with property
tests.

**Compatibility distance.** A weighted sum of graph distance (parts, joints,
topology), trait distance from the combine, and species-marker distance. Two
creatures can mate only if their distance is below the speciation threshold. This
is what makes reproductive isolation, and therefore species, real.

## Combine harness

Every new genome is evaluated once in real physics, alone, in short trials. The
results are cached on the genome's content hash and are what the world tier runs
on afterward. This is the mechanism that makes a 3D-bodied ecosystem affordable.

Trials, each with a fixed duration and a fixed seed derived from the genome hash:

1. **Stand.** Can it hold itself up, and at what metabolic cost? Early-abort gate:
   a body that cannot keep its root off the ground for two seconds skips the
   locomotion trials and is marked low-viability.
2. **Sprint** on flat ground: top speed, acceleration, energy per meter.
3. **Turn**: maximum sustained turn rate.
4. **Rough ground**: speed over procedurally bumpy terrain.
5. **Slope**: steepest grade it can ascend.
6. **Swim**: speed in a buoyant, high-drag domain.
7. **Strike**: peak impulse delivered to a test object within reach.
8. **Carry**: how much added mass it can move at half its sprint speed.
9. **Reach and bulk**: maximum reach height, body volume, total mass, limb count,
   symmetry index, gait frequency from the dominant FFT component of its vertical
   motion.

Trial 1 additionally records the isometric cost of standing, which is how a
muscle-actuated body pays for posture. Trial 7's strike impulse comes from muscle
force alone, so reach and leverage matter as much as cross-section.

Output per genome: a trait vector, a viability score, total muscle work and
isometric cost per trial, and a compressed replay of the sprint and strike trials
for the viewer. Trials run in
parallel across genomes via rayon, never in parallel inside one trial.

**Free-energy detector.** After each trial step, audit total mechanical energy
against the work the muscles could have done, which is the sum over muscles of
force times contraction distance, bounded by each muscle's area and the
length-tension curve. Passive muscle stiffness is a spring and must be included on
both sides of the ledger, since an evolved creature will otherwise try to pump
energy out of it. Flag any
genome whose energy rose beyond tolerance, mark it `exploit_suspect`, exclude it
from the archive, and log it to a table the viewer exposes. Evolved 3D creatures
reliably find solver bugs; this is the guard, and it has a test that a deliberately
constructed exploit genome is caught.

## World model

Runs at a fixed tick (default 10 ticks per simulated hour) on the trait tier, with
no physics. Physics only runs for combines and for the handful of creatures the
camera is currently inspecting in body view.

**Terrain** is a procedurally generated heightmap with a water level, a
temperature field varying with latitude and elevation, and a moisture field.
**Seasons** cycle temperature and plant growth. **Tectonics**: over long
timescales, sea level and a slow uplift field change, fragmenting and reconnecting
habitats. This is the main driver of allopatric speciation and is not optional.
**Shocks**: rare, configurable die-offs (cold snap, drought, disease wave) that
clear incumbents and open niches.

**Plants** are a per-cell biomass field with their own evolvable traits: regrowth
rate, toughness (raises the energy cost of eating them), toxicity (requires
detoxification investment from the herbivore), and seed dispersal radius. Plant
genomes evolve by the same selection process at the cell level. Herbivory must be
an arms race, not a fixed resource.

**Nutrient loop is closed.** A soil nutrient field feeds plants; eating transfers
energy at a loss; death produces a carcass that scavengers can eat and that
otherwise decays into soil. Nothing is created or destroyed except a configurable
solar input into plant growth. The conservation audit is an acceptance test.

**Metabolism.** Basal rate scales with mass, with total muscle cross-section, and
with organ investment (sensors, immune, brain size). Movement costs the
combine-measured energy per meter scaled by terrain and by load; standing still
still costs the measured isometric rate, so a large-muscled creature pays for its
strength every tick whether it uses it or not. Attacking, gestating, growing, and emitting pheromone all
cost. Starvation reduces health before it kills.

**Predation and injury.** An attack resolves from the attacker's strike impulse,
the defender's mass and armor (derived from part density and dimensions), and both
creatures' speeds. Outcomes are damage, injury (a persistent trait penalty), or
death. Eating flesh pays according to the diet gene. Partial consumption leaves a
carcass.

**Pathogens** are a small evolving population of their own with transmissibility,
virulence, and an antigen vector; hosts have immune investment and resistance
genes. Red Queen dynamics are the designed answer to long-run stagnation.

**Pheromones**: four diffusing scalar channels on a grid, with decay. Creatures
emit on their genome's channel and sense gradients. Trails, herding, and alarm
behavior are expected to emerge; none of them is hard-coded.

**Lifecycle.** Creatures are born small; traits scale with growth so juveniles are
slow and vulnerable. Maturity unlocks reproduction. Senescence degrades traits
after an evolvable onset age. Reproduction is sexual: a mature creature with
surplus energy seeks a mate within compatibility distance, mate choice applies the
preference genes to observable traits, and the offspring genome is crossover plus
mutation. Offspring size and clutch size are evolvable, which opens the r/K axis.

## Speciation, phylogeny, and the archive

**Speciation.** Cluster the living population by compatibility distance each
epoch. A cluster that persists beyond a minimum size and duration is registered as
a species with an auto-generated binomial name, a founding timestamp, a founding
genome, and a parent species. Extinction is recorded when its last member dies.

**Phylogeny.** Every birth records parents. The species tree, with founding and
extinction events, branch population over time, and a representative genome and
replay per branch, is persisted and browsable.

**MAP-Elites archive.** A discretized grid over behavior descriptors: part count,
limb count, total mass, height, symmetry index, gait frequency, and aquatic
capability. Each cell holds the highest-viability genome found for that cell, with
its traits and its sprint replay. The archive is filled from every combine,
including creatures that natural selection then deletes, so weird lineages are
preserved. It is the browsable periodic table of body plans, it is the project's
main defense against stagnation, and it is the primary artifact a visitor is shown.

## Persistence and events

SQLite (bundled) holds: `genome` (hash, blob, traits, viability, discovery time,
parents), `creature` (lifespan, species, cause of death, offspring count),
`species`, `phylogeny_edge`, `event`, `timeseries` (population, biomass, mean
traits, species count, sampled at a configurable interval), `archive_cell`,
`exploit_suspect`, and `mutation`.

The `mutation` table is a v1 requirement even though nothing in v1 reads it, and
it is what makes phase three possible: one row per mutation event, recording the
operator name, parent genome hash, child genome hash, the child's combine traits
and viability score, whether the child was ever born into the world, and how long
it and its lineage survived. Write it for every mutation, including children that
die before reproducing. Rows are small and this is the project's primary dataset. World state snapshots are postcard blobs written on an interval
and at shutdown. Replays are compressed trajectory files referenced by id.

**Event bus.** The simulation emits typed events: species born, species extinct,
first predation, first swimmer, first flyer-adjacent gait, first pheromone-guided
hunt, new archive cell filled, population crash, mass extinction, record lifespan,
record size, exploit detected. Consumers are the store, the CLI, and the viewer's
auto-director, which can cut the camera to the subject of a notable event and keep
a highlight list the user can jump back to.

## Viewer

A Bevy application that attaches to a running daemon or opens a world file
read-only. Mobile is irrelevant; target a desktop window, keyboard and mouse,
and a dark theme.

- **World view.** 3D terrain with creatures drawn as instanced proxy shapes at
  distance, upgraded to their full articulated body near the camera. Overlays for
  temperature, moisture, plant biomass, pheromone channels, species coloring, and
  predation events. Free camera, follow-creature camera, and auto-director camera.
- **Creature inspector.** The body in 3D with joints highlighted, the morphology
  graph, the trait radar against species and global medians, live motor-brain
  activations and ethology-brain action preferences, energy and health, age,
  lineage links to parents and offspring, and the combine replays.
- **Phylogeny.** An interactive species tree with time on one axis and population
  as branch thickness; click a branch to open its representative creature.
- **Archive browser.** The MAP-Elites grid, two descriptor axes at a time with
  selectors for the rest, each cell showing its creature and playing its replay on
  hover.
- **Charts.** Population, biomass by trophic level, species count, mean and
  variance of key traits, archive coverage, all over the full run.
- **Time control.** Pause, speed multipliers, run-headless-then-scrub, and jump to
  any event in the highlight list.
- **Clip export.** Write a PNG sequence of the current camera over a time range
  into `exports/`, plus an ffmpeg command line in a sidecar text file. Do not
  depend on ffmpeg being installed.

## Designer

A 3D editor for building a creature by hand: add bones, drag attachment points,
set joint type and limits, draw muscles between two points on reachable bones and
set their cross-section, set recursion counts, toggle reflection, attach sensors,
set life-history genes. Muscles render as lines that thicken with cross-section
and highlight on activation during a test run. A live validity panel shows part count, mass,
metabolic cost, and bound violations. One button runs the combine and shows the
trait vector and the sprint replay. Another releases the creature into an open
world at a chosen location, as a founding population of a configurable size.
Save, load, and copy-as-share-string.

Ship a starter zoo of at least eight hand-authored presets that double as test
fixtures: grazer, sprinter, ambush predator, scavenger, burrower, swimmer, serpent
(deep recursion, no limbs), and a radially symmetric drifter.

## CLI

```
bestiary world new --seed S --preset <scenario> --out world.db
bestiary world run world.db [--until TICKS] [--speed max] [--snapshot-every N]
bestiary world attach world.db        # prints the IPC endpoint for the viewer
bestiary world status world.db
bestiary combine <genome-file|share-string> [--json] [--replay out.rpl]
bestiary archive ls world.db [--sort coverage|viability] [--export dir/]
bestiary phylo world.db [--format dot|json]
bestiary replay <id> [--export png-seq]
bestiary genome mutate|cross|validate|show
bestiary bench [--report]
```

Human-readable by default, `--json` everywhere.

## Determinism rules

Binding. These are the rules that make replays, bug reports, and the test suite
possible.

1. Fixed timestep everywhere. Physics at 1/240 s with substepping; world at its
   own fixed tick. No wallclock, no frame-rate coupling, ever.
2. All randomness comes from `rand_chacha` streams seeded from an explicit tuple
   (world seed, subsystem id, entity id, tick). No thread-local RNG, no `random()`,
   no hashing-based iteration order affecting results.
3. Rapier with `enhanced-determinism`, single-threaded within a trial. Parallelism
   is across trials only, and results are merged in a fixed, sorted order before
   anything observes them.
4. Deterministic collections only: `BTreeMap` and `BTreeSet`, or sorted `Vec`, for
   anything iterated in simulation logic. `HashMap` is allowed only for caches
   whose iteration order never reaches the simulation.
5. No floating-point reliance on platform transcendentals in simulation paths: use
   `libm` consistently so the same code gives the same result on every target.
6. Bitwise determinism is promised within one platform and one binary, not across
   platforms. Cross-platform reproduction is by replay file, not by re-simulation.
   Say this in the RUNBOOK rather than pretending otherwise.
7. State hashing: a canonical `state_hash(tick)` over the full world state, used by
   the determinism tests and printed by `bestiary world status`.

## Milestones

A milestone is done when its tests pass, `make check` is green, the commit is
made, and its boxes in `PLAN.md` are ticked.

| Milestone | Build | Done when |
| --- | --- | --- |
| M0 Scaffold | Workspace, crates, `Makefile`, pinned deps, `PLAN.md` with every task in this spec, `ASSUMPTIONS.md`, `.gitignore`, CI-equivalent `make check` | `make check` passes on an empty workspace; first commit |
| M1 Physics core and the gate | Rapier integration, fixed timestep, seeded RNG plumbing, the muscle force model (contraction-only, length-tension, passive stiffness), a hand-built test body that walks under muscle actuation, the free-energy audit, and the criterion throughput benchmark | `make bench` reports creature-seconds of physics per wallclock second and meets the budget below; determinism test passes on repeated identical trials |
| M2 Genome | Morphology graph of bones and joints, muscles with length-tension and the genesis guarantee, bounded expansion, body instantiation, motor brain with mirrored topology, all mutation operators, crossover, compatibility distance, serialization and share strings, the `mutation` record | Property tests: mutation and crossover always produce valid, bounded genomes with every actuated axis spanned by a muscle; round-trip is exact; a duplicated subtree produces a working mirrored limb with its muscles intact |
| M3 Combine harness | All nine trials, early-abort gate, trait extraction, content-hash cache, replay recording, rayon parallelism with fixed-order merge | Cache hit equals recompute exactly; parallel results equal serial results; the deliberate exploit genome is flagged; preset zoo traits are stable across runs |
| M4 World core | Terrain, climate, seasons, plants with evolvable traits, closed nutrient loop, metabolism, agents driven by ethology brain and traits, foraging, growth, senescence, death | Energy and nutrient conservation hold to tolerance over 1M ticks; five seeds run without population collapse or explosion |
| M5 Ecology | Predation and injury, scavenging, pathogens, pheromones, tectonics and sea level, shocks | Trophic levels all persist for 1M ticks at the tuned defaults on at least three of five seeds; a herbivore-only world with toxic plants shows the arms race in trait time series |
| M6 Reproduction and speciation | Sexual reproduction, mate choice, compatibility threshold, species registration and naming, phylogeny, SQLite store, event bus | At seed S, at least three species coexist at tick N and the phylogeny records a split following a tectonic fragmentation |
| M7 Archive | MAP-Elites grid, descriptors, elite selection, coverage metric, archive persistence and export | Archive coverage exceeds the floor below at seed S; every cell's stored genome re-combines to its recorded traits |
| M8 Viewer | Bevy app, world view with LOD, inspector, phylogeny, archive browser, charts, time control, replay scrubbing, auto-director, clip export | Viewer attaches to a running daemon and to a world file; headless render smoke test passes; attaching does not change `state_hash` |
| M9 Designer and zoo | Editor, validity panel, combine-from-editor, release-into-world, save/load/share, the eight presets | Each preset round-trips, combines, and survives its first generation in the tuned default world |
| M10 Ship | `make dist` for the host platform, `RUNBOOK.md`, `TUNING.md`, final `ASSUMPTIONS.md` pass, long-run validation, fresh-clone test | Completion checklist fully true; final commit tagged `v0.1.0` |

## Performance budget

Measured by `make bench` on the build machine, reported in the benchmark output,
and gated in M1:

- Combine: a full nine-trial evaluation of a 12-part creature in under 2 seconds
  of single-core wallclock.
- World tier: at least 5,000 live creatures stepping at 10,000 world ticks per
  minute on a single core, scaling with cores for combines.
- Viewer: 60 fps with 2,000 visible proxy creatures and 20 full articulated bodies
  near the camera.

If the gate fails, cut trial durations, reduce trial count, coarsen the world tick,
or increase proxy LOD aggressiveness, in that order, and record it.

## Testing requirements

- **Determinism**: identical seeds produce identical `state_hash` at ticks 1k,
  100k, and 1M; serial and parallel combines agree; a viewer attach/detach cycle
  does not perturb the hash; a replay reproduces its recorded trajectory.
- **Conservation**: total energy and total nutrient are conserved to within
  epsilon over 1M ticks, accounting for solar input and metabolic dissipation.
- **Exploit detection**: the constructed free-energy genome is flagged and
  excluded; a normal genome is not.
- **Genome property tests** (proptest): bounded expansion under any mutation
  sequence; validity after crossover; exact serialization round-trip; compatibility
  distance is symmetric and zero only for identical genomes.
- **Open-endedness smoke**: at seed S after the configured generation count, the
  archive has filled at least 15% of reachable cells, the maximum part count
  exceeds the founding population's, and at least three species are alive.
- **Stagnation guard**: across five seeds, trait variance and archive coverage are
  both still increasing at the end of the long run.
- **World stability**: five seeds, 1M ticks, no collapse to zero and no unbounded
  growth; `make longrun` is the harness.
- **Unit tests** for every module, including negative cases for every config
  schema.
- **Viewer**: headless smoke test that the app builds, opens a world file, and
  renders one frame; unit tests for formatting, chart binning, and the IPC codec.
- `make check` runs fmt, clippy with warnings denied, the workspace test suite, and
  a release build. Coverage floor of 80% for `bestiary-core`, `bestiary-sim`, and
  `bestiary-world`.

## Tuning

Every constant the user might want to change lives in `config/world.ron` with a
comment per field, loaded at world creation and recorded into the world file so a
run is reproducible from its own file. `TUNING.md` documents what each knob does,
which direction to move it for a given symptom, and names the four failure modes
with their fixes: the sessile-sipper equilibrium (make stillness lose: patchier
regrowth, stronger seasons), monoculture (raise mutation floor, fragment the map
harder), predator-prey oscillation to extinction (raise refuge availability, lower
attack success at parity), and stagnation after a few thousand generations (raise
pathogen virulence drift, widen archive reseeding).

Muscle constants get their own documented block: force per unit cross-section,
the length-tension curve's width, maximum contraction fraction, passive stiffness
and damping, maximum span in joints, isometric cost coefficient, and the genesis
guarantee's attachment randomization. These are the knobs that decide whether
early populations flail or walk, so `TUNING.md` names them first and gives the
symptom for each (creatures that vibrate: stiffness or force too high; creatures
that collapse under their own weight: force per area too low or mass too high;
creatures that never leave the ground: span too short to produce leverage).

**Archive reseeding** is a tunable the user will want: on a configurable interval,
reintroduce an elite from a sparsely populated archive cell into the world. It is
off by default, because the purist run is natural selection only, but it is the
main lever for keeping a long run interesting.

## Phase two (specify the boundaries in v1, build later)

- **Scenario and rule plugins**: world rules, selection overrides, and scenarios as
  data plus an optional Lua hook behind a feature flag. v1 must route all world
  rules through one trait so this is additive.
- **Director mode**: the player authors the selection pressure and the game scores
  how the population gamed it, with a reward-hacking scoreboard.
- **Experiment runner**: parameter sweeps across seeds, run headless across all
  cores, emitting a dataframe and plots. v1 must make `bestiary world run`
  scriptable and its outputs machine-readable so this is a wrapper.
- **Comparative arena**: head-to-head trials between any two saved creatures.
  v1 must expose the trial harness as a public API.

## Phase three: mutational viability (research mode)

The open question this project is actually built to ask: how viable are mutations
to a body, which operators produce the rare large wins, and does a lineage evolve
toward mutational robustness over time? This is the hardest part of the project
and the most interesting, so v1 does not attempt it. v1's job is to make it
possible without a rewrite, which it does through two requirements that are
already binding above: the `mutation` table records every mutation event with its
operator, parents, combine result, and survival outcome; and the combine harness
is a deterministic, cacheable, parallel pure function from genome to traits.

What phase three adds, specified now only so the seams are in the right place:

- **Probe mode.** Take one genome, generate N mutants per operator, combine them
  all, and report the distribution of fitness effects per operator: the share
  lethal (fails the stand gate), the share near-neutral, and the tail of
  improvements. At the v1 budget of roughly two core-seconds per combine, a
  thousand-mutant probe is about half an hour on one core and minutes across a
  machine, so this is affordable; a landscape swept across many genomes and many
  generations is not, and should be sampled rather than exhausted.
- **Robustness and evolvability over time.** For sampled ancestors along a
  lineage, measure robustness (share of mutants that stay viable) and evolvability
  (how much of the archive's descriptor space a single mutation can reach). The
  hypothesis worth testing is that robustness rises over a long run while
  evolvability does not collapse, which is canalization, and that recursion-count
  and subtree-duplication mutations carry a fatter improvement tail than parameter
  jitter does.
- **Epistasis probes.** Apply pairs of mutations that are each near-neutral alone
  and measure whether the combination is not, which is the mechanism behind the
  long neutral walks that precede sudden morphological jumps.
- **Viewer support.** A landscape panel showing the DFE per operator for the
  selected creature, and lineage plots of robustness and evolvability over time.

Treat the output as a write-up, not a feature: the deliverable is a set of plots
and a claim about this simulator's evolutionary dynamics, which is a genuinely
publishable shape of result for an artificial-life system.

## Completion checklist

The run ends only when every line is true. Check them yourself; do not report done
on the strength of the milestone commits alone.

- [ ] `SPEC.md`, `AGENTS.md`, `PLAN.md` (all boxes ticked), `ASSUMPTIONS.md`,
      `RUNBOOK.md`, `TUNING.md` exist and are current.
- [ ] `make check` passes on a fresh clone in a temp directory.
- [ ] `make bench` meets every number in the performance budget, and the numbers
      are recorded in `RUNBOOK.md`.
- [ ] `make longrun` passes: five seeds, 1M ticks, stability, conservation, and
      determinism assertions all green.
- [ ] `bestiary world new`, `run`, `status`, `combine`, `archive ls`, `phylo`, and
      `replay` all work against a fresh world without error.
- [ ] The viewer builds, attaches to a running daemon, opens a world file
      read-only, and every panel renders real data from a long-run world.
- [ ] The designer builds a creature, combines it, and releases it into a running
      world; all eight presets round-trip and survive their first generation.
- [ ] At the documented demo seed, a run reaches at least three coexisting
      species, at least 15% archive coverage, and at least one species whose part
      count exceeds anything in the founding population. The seed and the expected
      figures are recorded in `RUNBOOK.md`.
- [ ] No simulation crate depends on `bevy`, and no simulation code uses
      wallclock time, thread-local RNG, or a `HashMap` in an order-sensitive path.
- [ ] No instantiated body has an actuated joint axis without a spanning muscle,
      under any sequence of mutations the property tests can produce, and no code
      path anywhere applies a joint motor torque.
- [ ] The `mutation` table is populated for every mutation event in a long run,
      including children that never reproduced, with operator, parent and child
      hashes, combine traits, viability, and survival outcome.
- [ ] `make dist` produces a runnable binary pair for the host platform, and
      `RUNBOOK.md` gives the build command for all three platforms.
- [ ] `git status` is clean, `Cargo.lock` is committed, the final commit is tagged
      `v0.1.0`, and `var/`, `worlds/`, `exports/`, and `dist/` are gitignored.
- [ ] `RUNBOOK.md` covers: building on each platform, creating and running a
      world, attaching the viewer, leaving a run going for days, reading the
      phylogeny and archive, designing and releasing a creature, exporting clips,
      where the tuning knobs are, and the cross-platform determinism caveat.
