# Training mode — design

Date: 2026-10-15. Status: approved in conversation; awaiting written-spec review.

## Purpose

Take any body (a starter preset, a designer draft, or a creature from a world),
pick a physical task, and watch a population of copies learn that task in real
3D physics over successive generations, in the manner of Keiwan's Evolution.
Keep the result: save it, open it in the designer, or release it into a world.

Motivation, measured on 2026-10-15 with the demo config: the three distinctive
starter bodies are effectively motionless (swimmer swim speed 0.011 m/s, serpent
ground speed 0.001 m/s, radial drifter 0.000 m/s), and the five 13-part presets
are variants of one hand-tuned hexapod (the "sprinter" is slower than the
grazer). The bodies are valid; nothing has learned to drive them.

## Success criteria (mechanical)

Per AGENTS.md rule 8 these are the only criteria the build claims:

1. Same body, task, settings and seed produce bit-identical generation reports;
   serial and parallel evaluation produce identical reports.
2. Best fitness never decreases across generations.
3. A genome flagged by the energy audit is never selected as an elite or champion.
4. In brain-only mode, every trained genome's body (all genes except motor brains)
   is byte-identical to the source.
5. `training_improves_zeroed_grazer_sprint`: the grazer body with all motor
   weights zeroed improves its Sprint fitness by at least a fixed margin within
   a fixed generation budget at a fixed seed (thresholds chosen from a measured
   run and recorded in the test).
6. `training_improves_swimmer_swim`: the shipped swimmer improves its Swim
   fitness by at least a fixed margin within a fixed generation budget.

## Approach

Brain-only training on a fixed body is the default, matching Evolution. An
off-by-default toggle also allows body mutation using the existing morphology
operators. Training inside the world ecosystem was rejected: the world tier runs
on traits, not physics, so there is nothing to watch.

## Engine: `bestiary-sim::training`

Pure simulation code: no UI, no async, no dependency on `bestiary-world`, so the
sim/viewer boundary holds.

```rust
pub enum Task { Sprint, Swim, Turn, Slope, Rough, Strike }

pub struct TrainingSettings {        // from config/training.ron
    pub population: usize,           // default 50
    pub elite_fraction: f32,         // default 0.2
    pub generations: u32,            // default 50 (CLI/headless budget)
    pub slope_angle: f32,            // radians, Slope task only
    pub weight_rate: f32,            // per-connection mutation probability
    pub weight_sigma: f32,           // weight jitter scale
    pub oscillator_sigma: f32,       // frequency/phase/bias jitter scale
    pub activation_rate: f32,        // per-bone activation change probability
    pub body_mutation: bool,         // default false
}

pub struct Training { /* source genome, task, settings, seed, generation, population */ }
impl Training {
    pub fn new(source: Genome, task: Task, settings: TrainingSettings,
               config: &Config, seed: u64) -> Result<Self, String>;
    pub fn next_generation(&mut self, parallel: bool) -> Result<GenerationReport, String>;
}

pub struct GenerationReport {
    pub generation: u32,
    pub best: f32, pub median: f32, pub worst: f32,
    pub champion: Genome,            // best of this generation
    pub champion_replay: Replay,
    pub runs: Vec<RunSummary>,       // every individual: hash, fitness, exploit flag
    pub replays: Vec<Replay>,        // every individual, latest generation only
}
```

**Fitness** comes from one existing physics trial per individual:

| Task | Trial | Fitness |
|---|---|---|
| Sprint | `TrialKind::Sprint` | net centre-of-mass speed, m/s |
| Swim | `TrialKind::Swim` | swim speed, m/s |
| Turn | `TrialKind::Turn` | sustained turn rate |
| Slope | `TrialKind::Slope` at `slope_angle` | uphill progress, m |
| Rough | `TrialKind::Rough` | speed over rough ground, m/s |
| Strike | `TrialKind::Strike` | peak impulse |

Exploit-suspect or failed trials get fitness negative infinity.

**Replay recording.** Today only Sprint and Strike record replays. Add
`trial_recorded(g, c, kind, slope, record: bool)`; the existing `trial` keeps
its current behaviour, so combine results, caches and hashes are unchanged.

**Selection.** Deterministic truncation with elitism. Sort by fitness
descending, ties broken by ascending genome hash. Keep the top
`ceil(population × elite_fraction)` unchanged. Fill the rest of the population
with mutants: child `k` takes elite `k mod elites` as its parent, with RNG
stream `(seed, TRAINING_SUBSYSTEM, k, generation)`. Generation 0 is the source
plus `population − 1` mutants of it.

**Controller mutation** is a new `Genome::mutate_controller` in `bestiary-core`.
It changes only motor-brain genes: per-connection weight jitter, per-bone
oscillator frequency, phase and bias jitter, and occasional activation changes.
It is not a new `Operator` variant, because that would change
`Operator::ALL.len()` and therefore the genome encoding and every saved world.
The ethology brain and life-history genes are untouched. With `body_mutation`
on, each child also receives one existing morphology operator drawn from the
seeded stream, and the genesis guarantee applies as usual.

**Parallelism.** Individuals are evaluated with rayon across genomes and merged
in index order before anything observes them. This is the same rule as the
combine harness. No wall-clock time is used anywhere.

## Configuration

Training settings live in a new documented `config/training.ron`, embedded as
offline defaults and overridable with `--training PATH`. This mirrors
`config/viewer.ron`. They are not world parameters, so world files, trait
caches and state hashes are unaffected. The file gets negative schema tests.

## CLI

```
bestiary train <genome|preset> --task sprint|swim|turn|slope|rough|strike
               [--generations N] [--population P] [--seed S]
               [--body-mutation] [--training PATH]
               [--out trained.genome] [--replay best.rpl] [--json]
```

It prints one line per generation (best, median, worst), or JSON lines with
`--json`. It writes the final champion genome and its replay. Training is
headless and scriptable, and the tests drive it this way.

## Viewer: Training panel

A new `Tab::Training`.

- **Setup:** source picker (starter preset, current designer draft, or selected
  creature), task picker, population, body-mutation toggle, Start/Pause/Reset.
- **Arena:** the latest generation's runs drawn ghosted side by side, with the
  champion drawn solid, using the existing procedural anatomy. Earlier
  generations keep only their champion replay, which bounds memory.
- **Fitness chart:** best and median per generation.
- **Generation scrubber:** replay any generation's champion.
- **Actions:** Save genome, Open in designer, Release into world. Release reuses
  the designer's existing release request.
- **Execution:** a background worker thread owns the `Training` and sends
  `GenerationReport`s over the existing job/message channel pattern. The UI never
  blocks, and Pause takes effect between generations.

The UI stays plain egui in the current theme; the visual overhaul is a separate,
later project.

## Testing

- **Unit tests in `training`:** determinism, serial vs. parallel, monotone best,
  exploit exclusion, brain-only body invariance, selection tie-breaking, and
  generation-0 construction.
- **Core:** `mutate_controller` changes only motor genes, keeps the genome valid,
  and is deterministic per seed.
- **Config:** a valid `training.ron` parses; out-of-range values are rejected.
- **Acceptance:** success criteria 5 and 6, release profile, kept within the
  normal test-suite budget (small population and generation counts).
- **CLI:** `bestiary train` round-trips the output genome and replay; JSON lines
  parse.
- **Viewer:** unit tests for the fitness-chart series and panel state; the
  headless render smoke test opens the Training tab.
- **Gates:** `make check` passes. `make bench` is re-run to confirm combine,
  world and viewer budgets are unchanged.

## Performance

A Sprint trial is about 4 simulated seconds. At the per-creature-second cost
measured on this host, a population of 50 is about 8 core-seconds, or about one
wall-clock second on 20 threads. The training panel adds at most 50 ghosted
bodies to the scene. The viewer's 60 fps gate is measured on the existing
benchmark scene and is unaffected.

## Out of scope

- Retraining the shipped presets in place. This is a follow-up that changes the
  M9 fixtures and needs their acceptance re-run.
- New tasks such as jumping.
- The viability formula's preference for cheap standing over locomotion. This is
  a real, separate issue.
- The UI overhaul.
