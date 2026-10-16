# Training Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Evolve a body's motor brain for a chosen physics task over generations, headless (CLI) and live in a new viewer Training panel, in the manner of Keiwan's Evolution.

**Architecture:** A pure `bestiary-sim::training` engine evaluates a population with one existing physics trial each (rayon across genomes, merged in index order), ranks deterministically, and breeds the next generation using a new controller-only mutation in `bestiary-core`. The CLI wraps the engine. The viewer runs it on a worker thread, renders the latest generation's recorded poses side by side, and charts fitness.

**Tech Stack:** Rust 1.85.1 workspace; rapier3d (existing), rayon (existing), ron/serde (existing), Bevy 0.15.3 + bevy_egui 0.33.0 (viewer only). No new dependencies.

**Spec:** `docs/superpowers/specs/2026-10-15-training-mode-design.md`

## Global Constraints

- No runtime network, no downloaded assets (AGENTS.md rule 2).
- Determinism is binding. Randomness comes only from `bestiary_core::rng(seed, subsystem, entity, tick)`. No wall-clock time in simulation code. Use `BTreeMap`/sorted `Vec` for anything iterated in simulation logic. Use `libm` for transcendentals in simulation paths.
- Simulation crates (`bestiary-core`, `bestiary-sim`) contain no rendering, UI or async (AGENTS.md rule 11).
- No new crates. Exact pins stay as they are, and `Cargo.lock` stays committed (rule 9).
- Every tunable constant lives in one documented config file (rule 8). Training uses `config/training.ron`.
- Every acceptance criterion has a test with the criterion's name in it (rule 7). The names are `training_improves_zeroed_grazer_sprint` and `training_improves_swimmer_swim`.
- `Operator::ALL` must not change length; that would change genome encoding.
- Existing `trial()` behaviour, combine results, cache keys and world state hashes must be unchanged.
- Quality gate at the end of each task: `./scripts/cargo fmt --all -- --check`, `./scripts/cargo clippy --workspace --all-targets --locked -- -D warnings`, and the touched crates' tests. Full `make check` before the final commit.

## Review Focus

1. **Degenerate settings** (population 0 or 1, elite fraction 0 or ≥1, non-finite sigmas): expect a clear validation error, never a panic or an empty population. Pinned in Task 3.
2. **A source body that can't be instantiated or fails every trial:** `Training::new` or the first generation returns `Err`, and the viewer shows the message. Pinned in Task 4.
3. **A body that can't move at all** (every fitness equal, e.g. 0.0): ranking is still total and deterministic (hash tie-break), and a champion is still reported. Pinned in Task 4.
4. **NaN or exploit fitness:** ranked below every finite, clean individual and never chosen as champion. Pinned in Task 4.
5. **Pause or Reset while a generation is computing:** the in-flight generation finishes, then the worker stops or discards. No report from a discarded run reaches the panel. Pinned in Task 7.

---

### Task 1: Controller-only mutation in `bestiary-core`

**Files:**
- Create: `crates/bestiary-core/src/genome/controller.rs`
- Modify: `crates/bestiary-core/src/genome/mod.rs` (add `mod controller; pub use controller::*;` next to `mod genetics;`)
- Test: inline `#[cfg(test)] mod tests` in `controller.rs`

**Interfaces:**
- Produces:
  - `pub struct ControllerMutation { pub weight_rate: f32, pub weight_sigma: f32, pub oscillator_sigma: f32, pub activation_rate: f32 }`
  - `impl Genome { pub fn mutate_controller(&self, params: &ControllerMutation, seed: u64, entity: u64, tick: u64) -> Genome }`
  - `impl Genome { pub fn same_morphology(&self, other: &Genome) -> bool }`. True when every gene except each bone's `motor` brain is identical.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Limits;
    fn limits() -> Limits {
        Limits { max_parts: 32, max_depth: 12, max_mass: 100.0, min_size: 0.02, max_span: 2,
                 mutation_floor: 0.001, mutation_ceiling: 0.5, attachment_jitter: 0.2 }
    }
    fn params() -> ControllerMutation {
        ControllerMutation { weight_rate: 0.3, weight_sigma: 0.5, oscillator_sigma: 0.2, activation_rate: 0.1 }
    }
    #[test]
    fn controller_mutation_changes_only_motor_brains_and_stays_valid() {
        let g = Genome::starter(&limits());
        for entity in 0..50 {
            let child = g.mutate_controller(&params(), 7, entity, 3);
            assert!(child.same_morphology(&g), "body genes changed");
            assert_eq!(child.ethology, g.ethology);
            assert_eq!(child.life, g.life);
            assert_eq!(child.mutation_rates, g.mutation_rates);
            child.validate(&limits()).unwrap();
        }
    }
    #[test]
    fn controller_mutation_is_deterministic_and_seed_sensitive() {
        let g = Genome::starter(&limits());
        assert_eq!(g.mutate_controller(&params(), 7, 1, 3), g.mutate_controller(&params(), 7, 1, 3));
        assert_ne!(g.mutate_controller(&params(), 7, 1, 3), g.mutate_controller(&params(), 7, 2, 3));
        assert_ne!(g.mutate_controller(&params(), 7, 1, 3), g);
    }
    #[test]
    fn same_morphology_detects_body_changes() {
        let g = Genome::starter(&limits());
        let mut moved = g.clone();
        moved.bones[0].size[0] *= 1.1;
        assert!(!moved.same_morphology(&g));
        let mut rewired = g.clone();
        rewired.bones[0].motor.frequency += 0.5;
        assert!(rewired.same_morphology(&g));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `./scripts/cargo test -p bestiary-core --locked controller`
Expected: compile error, `ControllerMutation` / `mutate_controller` not found.

- [ ] **Step 3: Implement**

```rust
//! Motor-controller-only variation for physics training. It is not an
//! `Operator`: adding one would change `Operator::ALL` and the genome encoding.
use super::Genome;
use crate::brain::Activation;
use rand::Rng;

const CONTROLLER_SUBSYSTEM: u64 = 0x434f_4e54;

#[derive(Clone, Debug, PartialEq)]
pub struct ControllerMutation {
    /// Probability that each motor connection weight is perturbed.
    pub weight_rate: f32,
    /// Half-width of each of the two summed uniform weight perturbations.
    pub weight_sigma: f32,
    /// Half-width of per-bone oscillator frequency, phase and bias perturbations.
    pub oscillator_sigma: f32,
    /// Probability per bone that one motor neuron's activation is redrawn.
    pub activation_rate: f32,
}

impl Genome {
    pub fn mutate_controller(&self, p: &ControllerMutation, seed: u64, entity: u64, tick: u64) -> Genome {
        let mut g = self.clone();
        let mut r = crate::rng(seed, CONTROLLER_SUBSYSTEM, entity, tick);
        for bone in &mut g.bones {
            let motor = &mut bone.motor;
            for neuron in &mut motor.neurons {
                for c in &mut neuron.inputs {
                    if r.gen::<f32>() < p.weight_rate {
                        let d = r.gen_range(-p.weight_sigma..=p.weight_sigma)
                            + r.gen_range(-p.weight_sigma..=p.weight_sigma);
                        c.weight = (c.weight + d).clamp(-16.0, 16.0);
                    }
                }
            }
            let s = p.oscillator_sigma;
            motor.frequency = (motor.frequency + r.gen_range(-s..=s)).clamp(0.01, 10.0);
            motor.phase += r.gen_range(-s..=s) * std::f32::consts::PI;
            let n = r.gen_range(0..motor.neurons.len());
            motor.neurons[n].bias = (motor.neurons[n].bias + r.gen_range(-s..=s) * 4.0).clamp(-16.0, 16.0);
            if r.gen::<f32>() < p.activation_rate {
                let n = r.gen_range(0..motor.neurons.len());
                motor.neurons[n].activation = [Activation::Tanh, Activation::Sigmoid, Activation::Sin,
                    Activation::Abs, Activation::Threshold][r.gen_range(0..5)];
            }
        }
        g
    }

    pub fn same_morphology(&self, other: &Genome) -> bool {
        let strip = |g: &Genome| {
            let mut g = g.clone();
            for bone in &mut g.bones {
                bone.motor = crate::brain::Brain { neurons: vec![], frequency: 0.0, phase: 0.0 };
            }
            g
        };
        strip(self) == strip(other)
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `./scripts/cargo test -p bestiary-core --locked controller`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
./scripts/cargo fmt --all && ./scripts/cargo clippy -p bestiary-core --all-targets --locked -- -D warnings
git add crates/bestiary-core/src/genome/controller.rs crates/bestiary-core/src/genome/mod.rs
git commit -m "Controller-only genome mutation for training"
```

---

### Task 2: Recordable trials for every task

**Files:**
- Modify: `crates/bestiary-sim/src/combine.rs`. Rename the body of `pub fn trial(...)` to `pub fn trial_recorded(g, c, kind, slope, record: bool)`. Replace the line `let save = matches!(kind, TrialKind::Sprint | TrialKind::Strike);` with `let save = record;`. Re-add `trial` as a wrapper.
- Test: the `#[cfg(test)]` module in `combine.rs`

**Interfaces:**
- Produces: `pub fn trial_recorded(g: &Genome, c: &Config, kind: TrialKind, slope: f32, record: bool) -> Result<TrialResult, String>`
- Unchanged: `pub fn trial(g, c, kind, slope)`, which equals `trial_recorded(g, c, kind, slope, matches!(kind, TrialKind::Sprint | TrialKind::Strike))`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn recorded_trials_match_unrecorded_results_for_every_task() {
    let config = Config::bundled();
    let g = Genome::starter(&config.genome);
    for kind in [TrialKind::Sprint, TrialKind::Swim, TrialKind::Turn, TrialKind::Rough, TrialKind::Strike] {
        let plain = trial(&g, &config, kind, 0.0).unwrap();
        let recorded = trial_recorded(&g, &config, kind, 0.0, true).unwrap();
        let replay = recorded.replay.clone().expect("recorded replay");
        assert_eq!(replay.kind, kind);
        assert!(replay.frames.len() > 1);
        assert_eq!(TrialResult { replay: None, ..recorded }, TrialResult { replay: None, ..plain });
    }
    let slope = trial_recorded(&g, &config, TrialKind::Slope, 0.2, true).unwrap();
    assert!(slope.replay.is_some());
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `./scripts/cargo test -p bestiary-sim --locked recorded_trials`
Expected: compile error, `trial_recorded` not found.

- [ ] **Step 3: Implement**

```rust
pub fn trial(g: &Genome, c: &Config, kind: TrialKind, slope: f32) -> Result<TrialResult, String> {
    trial_recorded(g, c, kind, slope, matches!(kind, TrialKind::Sprint | TrialKind::Strike))
}
/// Like `trial`, but records poses for any kind when `record` is set. Recording
/// never changes the simulated result.
pub fn trial_recorded(
    g: &Genome,
    c: &Config,
    kind: TrialKind,
    slope: f32,
    record: bool,
) -> Result<TrialResult, String> {
    // ...existing body of `trial`, with `let save = record;`
}
```

- [ ] **Step 4: Run the sim tests (combine regressions must still pass)**

Run: `./scripts/cargo test -p bestiary-sim --locked`
Expected: all pass, including the existing cache/serial/parallel/zoo-stability tests.

- [ ] **Step 5: Commit**

```bash
git add crates/bestiary-sim/src/combine.rs
git commit -m "Record replays for any trial kind without changing results"
```

---

### Task 3: Training settings file and schema

**Files:**
- Create: `config/training.ron`
- Create: `crates/bestiary-sim/src/training/mod.rs` (settings and module root for now)
- Create: `crates/bestiary-sim/src/training/settings.rs`
- Modify: `crates/bestiary-sim/src/lib.rs` (add `pub mod training;`)
- Test: inline tests in `settings.rs`

**Interfaces:**
- Produces:
  - `pub struct TrainingSettings { pub population: usize, pub elite_fraction: f32, pub generations: u32, pub slope_angle: f32, pub weight_rate: f32, pub weight_sigma: f32, pub oscillator_sigma: f32, pub activation_rate: f32, pub body_mutation: bool }` (Serialize, Deserialize, Clone, Debug, PartialEq, `deny_unknown_fields`)
  - `impl TrainingSettings { pub fn parse(source: &str) -> Result<Self, String>; pub fn bundled() -> Self; pub fn validate(&self) -> Result<(), String>; pub fn elites(&self) -> usize; pub fn controller(&self) -> bestiary_core::genome::ControllerMutation }`

- [ ] **Step 1: Write `config/training.ron`**

```ron
(
    // Individuals evaluated per generation (2..=512).
    population: 50,
    // Fraction kept unchanged as parents of the next generation (0 < f < 1).
    elite_fraction: 0.2,
    // Default generation budget for headless runs; the viewer runs until paused.
    generations: 50,
    // Incline in radians for the Slope task.
    slope_angle: 0.2,
    // Probability that each motor connection weight is perturbed per child.
    weight_rate: 0.2,
    // Half-width of each of two summed uniform weight perturbations.
    weight_sigma: 0.6,
    // Half-width of oscillator frequency/phase/bias perturbations.
    oscillator_sigma: 0.15,
    // Probability per bone that one motor neuron's activation is redrawn.
    activation_rate: 0.05,
    // Also apply one morphology operator per child (off: Evolution-style fixed body).
    body_mutation: false,
)
```

- [ ] **Step 2: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_training_settings_are_valid() {
        let s = TrainingSettings::bundled();
        assert_eq!(s.elites(), 10);
        assert!(!s.body_mutation);
    }
    #[test]
    fn training_settings_reject_degenerate_values() {
        let base = TrainingSettings::bundled();
        let cases: Vec<fn(&mut TrainingSettings)> = vec![
            |s| s.population = 0,
            |s| s.population = 1,
            |s| s.population = 513,
            |s| s.elite_fraction = 0.0,
            |s| s.elite_fraction = 1.0,
            |s| s.elite_fraction = f32::NAN,
            |s| s.weight_rate = -0.1,
            |s| s.weight_rate = 1.5,
            |s| s.weight_sigma = f32::INFINITY,
            |s| s.oscillator_sigma = -1.0,
            |s| s.activation_rate = 2.0,
            |s| s.slope_angle = 1.6,
            |s| s.generations = 0,
        ];
        for (i, case) in cases.into_iter().enumerate() {
            let mut s = base.clone();
            case(&mut s);
            assert!(s.validate().is_err(), "case {i} accepted");
        }
    }
    #[test]
    fn training_settings_reject_unknown_fields() {
        let source = include_str!("../../../../config/training.ron").replace("population:", "populaton:");
        assert!(TrainingSettings::parse(&source).is_err());
    }
    #[test]
    fn small_population_keeps_at_least_one_elite_and_one_child() {
        let mut s = TrainingSettings::bundled();
        s.population = 2;
        s.elite_fraction = 0.9;
        assert_eq!(s.elites(), 1);
    }
}
```

- [ ] **Step 3: Run to verify they fail**

Run: `./scripts/cargo test -p bestiary-sim --locked training_settings`
Expected: compile error.

- [ ] **Step 4: Implement `settings.rs`, and `mod.rs` with `mod settings; pub use settings::*;`**

```rust
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainingSettings {
    pub population: usize,
    pub elite_fraction: f32,
    pub generations: u32,
    pub slope_angle: f32,
    pub weight_rate: f32,
    pub weight_sigma: f32,
    pub oscillator_sigma: f32,
    pub activation_rate: f32,
    pub body_mutation: bool,
}
impl TrainingSettings {
    pub fn parse(source: &str) -> Result<Self, String> {
        let s: Self = ron::from_str(source).map_err(|e| e.to_string())?;
        s.validate()?;
        Ok(s)
    }
    pub fn bundled() -> Self {
        Self::parse(include_str!("../../../../config/training.ron")).expect("bundled training settings")
    }
    pub fn validate(&self) -> Result<(), String> {
        let unit = |x: f32| x.is_finite() && (0.0..=1.0).contains(&x);
        if !(2..=512).contains(&self.population) {
            return Err("population must be 2..=512".into());
        }
        if !(self.elite_fraction.is_finite() && self.elite_fraction > 0.0 && self.elite_fraction < 1.0) {
            return Err("elite_fraction must be strictly between 0 and 1".into());
        }
        if self.generations == 0 {
            return Err("generations must be positive".into());
        }
        if !(self.slope_angle.is_finite() && (0.0..1.5).contains(&self.slope_angle)) {
            return Err("slope_angle must be in [0, 1.5) radians".into());
        }
        if !unit(self.weight_rate) || !unit(self.activation_rate) {
            return Err("rates must be in [0, 1]".into());
        }
        for (name, v) in [("weight_sigma", self.weight_sigma), ("oscillator_sigma", self.oscillator_sigma)] {
            if !(v.is_finite() && (0.0..=16.0).contains(&v)) {
                return Err(format!("{name} must be in [0, 16]"));
            }
        }
        Ok(())
    }
    /// At least one elite and at least one child.
    pub fn elites(&self) -> usize {
        ((self.population as f32 * self.elite_fraction).ceil() as usize).clamp(1, self.population - 1)
    }
    pub fn controller(&self) -> bestiary_core::genome::ControllerMutation {
        bestiary_core::genome::ControllerMutation {
            weight_rate: self.weight_rate,
            weight_sigma: self.weight_sigma,
            oscillator_sigma: self.oscillator_sigma,
            activation_rate: self.activation_rate,
        }
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `./scripts/cargo test -p bestiary-sim --locked training_settings small_population`
Expected: 4 passed.

- [ ] **Step 6: Commit**

```bash
git add config/training.ron crates/bestiary-sim/src/training crates/bestiary-sim/src/lib.rs
git commit -m "Training settings file with validated schema"
```

---

### Task 4: Training engine

**Files:**
- Create: `crates/bestiary-sim/src/training/engine.rs`
- Modify: `crates/bestiary-sim/src/training/mod.rs` (add `mod engine; pub use engine::*;`)
- Test: inline tests in `engine.rs`

**Interfaces:**
- Consumes: `Genome::mutate_controller`, `Genome::same_morphology` (Task 1), `trial_recorded` (Task 2), `TrainingSettings` (Task 3)
- Produces:
  - `pub enum Task { Sprint, Swim, Turn, Slope, Rough, Strike }` (Copy, Debug, PartialEq, Eq, Serialize, Deserialize), with `pub const ALL: [Task; 6]`, `pub fn name(self) -> &'static str`, `pub fn parse(s: &str) -> Option<Task>` and `pub fn kind(self) -> TrialKind`
  - `pub struct RunSummary { pub hash: [u8; 32], pub fitness: f32, pub exploit: bool }`
  - `pub struct GenerationReport { pub generation: u32, pub best: f32, pub median: f32, pub worst: f32, pub champion: Genome, pub champion_replay: Replay, pub runs: Vec<RunSummary>, pub genomes: Vec<Genome>, pub replays: Vec<Replay> }` (Clone, Debug, PartialEq). `genomes[i]`, `replays[i]` and `runs[i]` refer to the same individual, sorted by rank (best first).
  - `pub fn rank(runs: &[RunSummary]) -> Vec<usize>`
  - `pub struct Training`, with `pub fn new(source: Genome, task: Task, settings: TrainingSettings, config: Config, seed: u64) -> Result<Self, String>`, `pub fn next_generation(&mut self, parallel: bool) -> Result<GenerationReport, String>` and `pub fn generation(&self) -> u32`

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    fn small() -> TrainingSettings {
        TrainingSettings { population: 6, ..TrainingSettings::bundled() }
    }
    fn grazer(c: &Config) -> Genome {
        bestiary_core::genome::zoo(&c.genome).into_iter().find(|(n, _)| *n == "grazer").unwrap().1
    }
    fn run(seed: u64, parallel: bool, generations: u32) -> Vec<GenerationReport> {
        let c = Config::bundled();
        let mut t = Training::new(grazer(&c), Task::Sprint, small(), c, seed).unwrap();
        (0..generations).map(|_| t.next_generation(parallel).unwrap()).collect()
    }
    #[test]
    fn training_is_deterministic_and_serial_equals_parallel() {
        let a = run(3, true, 3);
        assert_eq!(a, run(3, true, 3));
        assert_eq!(a, run(3, false, 3));
        assert_ne!(a[2].champion, run(4, true, 3)[2].champion);
    }
    #[test]
    fn training_best_fitness_never_decreases() {
        let reports = run(5, true, 4);
        for pair in reports.windows(2) {
            assert!(pair[1].best >= pair[0].best, "{} < {}", pair[1].best, pair[0].best);
        }
        assert_eq!(reports.iter().map(|r| r.generation).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
    }
    #[test]
    fn brain_only_training_keeps_the_body_byte_identical() {
        let c = Config::bundled();
        let source = grazer(&c);
        for report in run(6, true, 3) {
            assert!(report.genomes.iter().all(|g| g.same_morphology(&source)));
            assert!(report.champion.same_morphology(&source));
        }
    }
    #[test]
    fn rank_breaks_ties_by_hash_and_puts_exploits_and_nan_last() {
        let s = |h: u8, fitness: f32, exploit: bool| RunSummary { hash: [h; 32], fitness, exploit };
        let runs = vec![s(9, 1.0, false), s(1, 5.0, true), s(2, f32::NAN, false), s(4, 1.0, false), s(3, 0.5, false)];
        // Equal fitness 1.0: hash 4 before hash 9. Exploit (raw 5.0) and NaN rank last, by hash.
        assert_eq!(rank(&runs), vec![3, 0, 4, 1, 2]);
        let still = vec![s(7, 0.0, false), s(2, 0.0, false), s(5, 0.0, false)];
        assert_eq!(rank(&still), vec![1, 2, 0]);
    }
    #[test]
    fn body_mutation_mode_completes_and_reports_valid_champions() {
        let c = Config::bundled();
        let settings = TrainingSettings { body_mutation: true, ..small() };
        let mut t = Training::new(grazer(&c), Task::Sprint, settings, c.clone(), 8).unwrap();
        for _ in 0..2 {
            let report = t.next_generation(true).unwrap();
            report.champion.validate(&c.genome).unwrap();
            assert!(report.best.is_finite());
        }
    }
    #[test]
    fn training_rejects_an_unbuildable_source() {
        let c = Config::bundled();
        let mut broken = grazer(&c);
        broken.root = broken.bones.len() + 5;
        assert!(Training::new(broken, Task::Sprint, small(), c, 1).is_err());
    }
    #[test]
    fn every_task_produces_a_recorded_generation() {
        let c = Config::bundled();
        for task in Task::ALL {
            let settings = TrainingSettings { population: 2, ..TrainingSettings::bundled() };
            let mut t = Training::new(grazer(&c), task, settings, c.clone(), 2).unwrap();
            let report = t.next_generation(true).unwrap();
            assert_eq!(report.replays.len(), 2);
            assert_eq!(report.champion_replay.kind, task.kind());
        }
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `./scripts/cargo test -p bestiary-sim --locked --lib training::engine`
Expected: compile error.

- [ ] **Step 3: Implement `engine.rs`**

```rust
//! Deterministic generational training of one body for one physics task.
use super::TrainingSettings;
use crate::combine::{trial_recorded, Replay, TrialKind, TrialResult};
use crate::config::Config;
use bestiary_core::genome::{Genome, Operator};
use rand::Rng;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

const TRAINING_SUBSYSTEM: u64 = 0x5452_4149;
/// Morphology operators used when `body_mutation` is on.
const BODY_OPERATORS: [Operator; 17] = [
    Operator::Parameter, Operator::AddBone, Operator::RemoveBone, Operator::AddJoint,
    Operator::RemoveJoint, Operator::JointType, Operator::Recursion, Operator::Reflection,
    Operator::DuplicateSubtree, Operator::PruneSubtree, Operator::AddMuscle, Operator::RemoveMuscle,
    Operator::MoveAttachment, Operator::MuscleArea, Operator::MuscleSpan, Operator::AddSensor,
    Operator::RemoveSensor,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Task { Sprint, Swim, Turn, Slope, Rough, Strike }
impl Task {
    pub const ALL: [Task; 6] = [Task::Sprint, Task::Swim, Task::Turn, Task::Slope, Task::Rough, Task::Strike];
    pub fn name(self) -> &'static str {
        match self { Task::Sprint => "sprint", Task::Swim => "swim", Task::Turn => "turn",
                     Task::Slope => "slope", Task::Rough => "rough", Task::Strike => "strike" }
    }
    pub fn parse(s: &str) -> Option<Task> { Task::ALL.into_iter().find(|t| t.name() == s) }
    pub fn kind(self) -> TrialKind {
        match self { Task::Sprint => TrialKind::Sprint, Task::Swim => TrialKind::Swim,
                     Task::Turn => TrialKind::Turn, Task::Slope => TrialKind::Slope,
                     Task::Rough => TrialKind::Rough, Task::Strike => TrialKind::Strike }
    }
    fn fitness(self, r: &TrialResult, c: &Config) -> f32 {
        match self {
            Task::Sprint => r.speed,
            Task::Swim | Task::Rough => r.distance / c.combine.trial_seconds,
            Task::Turn => r.turn_rate,
            Task::Slope => r.progress,
            Task::Strike => r.strike_impulse,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunSummary { pub hash: [u8; 32], pub fitness: f32, pub exploit: bool }

#[derive(Clone, Debug, PartialEq)]
pub struct GenerationReport {
    pub generation: u32,
    pub best: f32,
    pub median: f32,
    pub worst: f32,
    pub champion: Genome,
    pub champion_replay: Replay,
    pub runs: Vec<RunSummary>,
    pub genomes: Vec<Genome>,
    pub replays: Vec<Replay>,
}

/// Indices best-first. Clean finite fitness descending, then everything else
/// (exploit-suspect or non-finite) last; ties by ascending genome hash.
pub fn rank(runs: &[RunSummary]) -> Vec<usize> {
    let key = |r: &RunSummary| if r.exploit || !r.fitness.is_finite() { f32::NEG_INFINITY } else { r.fitness };
    let mut order: Vec<usize> = (0..runs.len()).collect();
    order.sort_by(|&a, &b| key(&runs[b]).total_cmp(&key(&runs[a])).then(runs[a].hash.cmp(&runs[b].hash)));
    order
}

#[derive(Clone)]
struct Individual { genome: Genome, run: RunSummary, replay: Replay }

pub struct Training {
    task: Task,
    settings: TrainingSettings,
    config: Config,
    seed: u64,
    generation: u32,
    pending: Vec<Genome>,
    elites: Vec<Individual>,
}

impl Training {
    pub fn new(source: Genome, task: Task, settings: TrainingSettings, config: Config, seed: u64) -> Result<Self, String> {
        settings.validate()?;
        source.validate(&config.genome)?;
        source.expand(&config.genome)?;
        crate::genome::Creature::new(source.clone(), &config)?;
        let mut t = Training { task, settings, config, seed, generation: 0, pending: vec![], elites: vec![] };
        t.pending = std::iter::once(source.clone())
            .chain((1..t.settings.population).map(|k| t.child(&source, k as u64)))
            .collect();
        Ok(t)
    }
    pub fn generation(&self) -> u32 { self.generation }

    fn child(&self, parent: &Genome, k: u64) -> Genome {
        let mut g = parent.mutate_controller(&self.settings.controller(), self.seed, k, self.generation as u64);
        if self.settings.body_mutation {
            let mut r = bestiary_core::rng(self.seed, TRAINING_SUBSYSTEM, k, self.generation as u64);
            let op = BODY_OPERATORS[r.gen_range(0..BODY_OPERATORS.len())];
            g = g.mutate(op, self.seed ^ k, self.generation as u64, &self.config.genome).0;
        }
        g
    }

    fn evaluate(&self, g: &Genome) -> Individual {
        let slope = if self.task == Task::Slope { self.settings.slope_angle } else { 0.0 };
        let hash = g.hash();
        match trial_recorded(g, &self.config, self.task.kind(), slope, true) {
            Ok(r) => Individual {
                genome: g.clone(),
                run: RunSummary { hash, fitness: self.task.fitness(&r, &self.config), exploit: r.energy.exploit_suspect },
                replay: r.replay.expect("recorded"),
            },
            Err(_) => Individual {
                genome: g.clone(),
                run: RunSummary { hash, fitness: f32::NEG_INFINITY, exploit: false },
                replay: Replay { kind: self.task.kind(), dt: crate::PHYSICS_DT, frames: vec![] },
            },
        }
    }

    pub fn next_generation(&mut self, parallel: bool) -> Result<GenerationReport, String> {
        let fresh: Vec<Individual> = if parallel {
            self.pending.par_iter().map(|g| self.evaluate(g)).collect()
        } else {
            self.pending.iter().map(|g| self.evaluate(g)).collect()
        };
        let mut all: Vec<Individual> = self.elites.drain(..).chain(fresh).collect();
        let runs: Vec<RunSummary> = all.iter().map(|i| i.run.clone()).collect();
        let order = rank(&runs);
        all = order.into_iter().map(|i| all[i].clone()).collect();
        let champion = all.first().ok_or("empty population")?;
        if !champion.run.fitness.is_finite() || champion.run.exploit {
            return Err("no individual completed a clean trial".into());
        }
        let finite: Vec<f32> = all.iter().filter(|i| !i.run.exploit).map(|i| i.run.fitness).filter(|f| f.is_finite()).collect();
        let report = GenerationReport {
            generation: self.generation,
            best: finite[0],
            median: finite[finite.len() / 2],
            worst: *finite.last().unwrap(),
            champion: champion.genome.clone(),
            champion_replay: champion.replay.clone(),
            runs: all.iter().map(|i| i.run.clone()).collect(),
            genomes: all.iter().map(|i| i.genome.clone()).collect(),
            replays: all.iter().map(|i| i.replay.clone()).collect(),
        };
        let elites = self.settings.elites();
        self.elites = all.into_iter().take(elites).collect();
        self.generation += 1;
        self.pending = (0..(self.settings.population - elites) as u64)
            .map(|k| self.child(&self.elites[k as usize % elites].genome, k))
            .collect();
        Ok(report)
    }
}
```

Note for the implementer: `finite` is sorted descending because `all` is ranked. `median` takes the upper-middle element, which is deterministic. `Training::new` rejects a source that `validate`, `expand` or `Creature::new` refuses; `training_rejects_an_unbuildable_source` pins this.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `./scripts/cargo test -p bestiary-sim --locked --lib training`
Expected: all training tests pass. If the determinism test is slow (>60 s), reduce `small()` to population 4.

- [ ] **Step 5: Commit**

```bash
./scripts/cargo fmt --all && ./scripts/cargo clippy -p bestiary-sim --all-targets --locked -- -D warnings
git add crates/bestiary-sim/src/training
git commit -m "Deterministic generational training engine"
```

---

### Task 5: Acceptance tests for improvement

**Files:**
- Create: `crates/bestiary-sim/tests/training.rs`

**Interfaces:**
- Consumes: `Training`, `Task`, `TrainingSettings` (Task 4)

- [ ] **Step 1: Write the measuring version of the tests**

```rust
use bestiary_core::genome::zoo;
use bestiary_sim::{config::Config, training::{Task, Training, TrainingSettings}};

fn preset(c: &Config, name: &str) -> bestiary_core::genome::Genome {
    zoo(&c.genome).into_iter().find(|(n, _)| *n == name).unwrap().1
}
/// Best fitness after `generations` generations at a fixed seed.
fn trained_best(source: bestiary_core::genome::Genome, task: Task, generations: u32) -> f32 {
    let c = Config::bundled();
    let settings = TrainingSettings { population: 20, ..TrainingSettings::bundled() };
    let mut t = Training::new(source, task, settings, c, 42).unwrap();
    let mut best = f32::NEG_INFINITY;
    for _ in 0..generations {
        let r = t.next_generation(true).unwrap();
        println!("{task:?} generation {} best {} median {}", r.generation, r.best, r.median);
        best = r.best;
    }
    best
}

#[test]
fn training_improves_zeroed_grazer_sprint() {
    let c = Config::bundled();
    let mut g = preset(&c, "grazer");
    for bone in &mut g.bones {
        for n in &mut bone.motor.neurons {
            for conn in &mut n.inputs { conn.weight = 0.0; }
        }
    }
    let source_speed = bestiary_sim::combine::trial(&g, &c, bestiary_sim::combine::TrialKind::Sprint, 0.0).unwrap().speed;
    let best = trained_best(g, Task::Sprint, 15);
    println!("zeroed grazer source speed {source_speed}");
    assert!(best >= source_speed + MARGIN_SPRINT, "best {best} vs source {source_speed}");
}

#[test]
fn training_improves_swimmer_swim() {
    let c = Config::bundled();
    let g = preset(&c, "swimmer");
    let r = bestiary_sim::combine::trial(&g, &c, bestiary_sim::combine::TrialKind::Swim, 0.0).unwrap();
    let source = r.distance / c.combine.trial_seconds;
    let best = trained_best(g, Task::Swim, 15);
    println!("swimmer source swim {source}");
    assert!(best >= source + MARGIN_SWIM, "best {best} vs source {source}");
}
```

Start with `const MARGIN_SPRINT: f32 = 0.0;` and `const MARGIN_SWIM: f32 = 0.0;` for the measuring run.

- [ ] **Step 2: Measure**

Run: `./scripts/cargo test --release -p bestiary-sim --test training --locked -- --nocapture`
Record the printed source and best values in `ASSUMPTIONS.md`.

- [ ] **Step 3: Set the margins from the measurement**

Set each margin to half the measured improvement, rounded down to two significant figures. Use a strictly positive margin. If either measured improvement is ≤ 0, stop and investigate before continuing; it means the training doesn't work for that body. Check whether the trial is too short for the oscillator frequency range, and whether weight sigma is too small to escape zero.

- [ ] **Step 4: Run to verify they pass, in the normal (non-release) test profile too**

Run: `./scripts/cargo test -p bestiary-sim --test training --locked`
Expected: 2 passed. If over about 90 s, lower the population to 12 or the generations to 10, and re-measure.

- [ ] **Step 5: Commit**

```bash
git add crates/bestiary-sim/tests/training.rs ASSUMPTIONS.md
git commit -m "Training acceptance: zeroed grazer sprint and swimmer swim improve"
```

---

### Task 6: `bestiary train` CLI

**Files:**
- Modify: `crates/bestiary-cli/src/main.rs` (help text; new `"train"` match arm)
- Test: `crates/bestiary-cli/tests/commands.rs` (new test)

**Interfaces:**
- Consumes: `Training`, `Task::parse`, `TrainingSettings::{bundled, parse}` (Tasks 3–4); the existing CLI helpers `genome`, `config`, `value`, `number`
- Produces: the command `bestiary train GENOME --task T [--generations N] [--population P] [--seed S] [--body-mutation] [--training PATH] [--out FILE] [--replay FILE] [--json]`. It prints one line per generation; with `--json`, each line is a JSON object `{"generation","best","median","worst","champion"}`. `--out` writes the champion's share string as text, which the existing `genome()` loader reads back.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn train_command_writes_a_loadable_champion_and_replay() {
    let dir = std::env::temp_dir().join(format!("bestiary-train-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("champion.genome");
    let replay = dir.join("champion.rpl");
    let run = Command::new(env!("CARGO_BIN_EXE_bestiary"))
        .args(["train", "grazer", "--task", "sprint", "--generations", "2", "--population", "4",
               "--seed", "3", "--json", "--out", out.to_str().unwrap(), "--replay", replay.to_str().unwrap()])
        .output().unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let lines: Vec<serde_json::Value> = String::from_utf8(run.stdout).unwrap()
        .lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1]["generation"], 1);
    assert!(lines[1]["best"].as_f64().unwrap() >= lines[0]["best"].as_f64().unwrap());
    let combined = cli(&["combine", out.to_str().unwrap(), "--json"]);
    assert_eq!(combined["genome"], lines[1]["champion"]);
    assert!(bestiary_sim::combine::Replay::decode(&std::fs::read(&replay).unwrap()).is_ok());
    let bad = Command::new(env!("CARGO_BIN_EXE_bestiary")).args(["train", "grazer", "--task", "fly"]).output().unwrap();
    assert!(!bad.status.success());
    std::fs::remove_dir_all(&dir).unwrap();
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `./scripts/cargo test -p bestiary-cli --test commands --locked train_command`
Expected: FAIL, unknown command.

- [ ] **Step 3: Implement the match arm (before `"genome" =>`)**

```rust
"train" => {
    let c = config(args)?;
    let source = genome(arg(args, 1)?, &c)?;
    let task = value(args, "--task").and_then(bestiary_sim::training::Task::parse)
        .ok_or("--task must be sprint|swim|turn|slope|rough|strike")?;
    let mut settings = match value(args, "--training") {
        Some(path) => bestiary_sim::training::TrainingSettings::parse(&std::fs::read_to_string(path)?)
            .map_err(|e| format!("training: {e}"))?,
        None => bestiary_sim::training::TrainingSettings::bundled(),
    };
    settings.population = number(args, "--population", settings.population)?;
    settings.generations = number(args, "--generations", settings.generations)?;
    settings.body_mutation |= args.iter().any(|a| a == "--body-mutation");
    settings.validate()?;
    let generations = settings.generations;
    let mut training = bestiary_sim::training::Training::new(source, task, settings, c, number(args, "--seed", 42u64)?)?;
    let mut last = None;
    for _ in 0..generations {
        let r = training.next_generation(true)?;
        let champion = hex(&r.champion.hash());
        if json {
            println!("{}", serde_json::json!({"generation": r.generation, "best": r.best, "median": r.median, "worst": r.worst, "champion": champion}));
        } else {
            println!("generation={} best={:.4} median={:.4} worst={:.4} champion={champion}", r.generation, r.best, r.median, r.worst);
        }
        last = Some(r);
    }
    let last = last.ok_or("no generations run")?;
    if let Some(path) = value(args, "--out") {
        std::fs::write(path, last.champion.to_share())?;
    }
    if let Some(path) = value(args, "--replay") {
        std::fs::write(path, last.champion_replay.encode()?)?;
    }
}
```

Also append to the help string: `\nbestiary train GENOME_OR_PRESET --task sprint|swim|turn|slope|rough|strike [--generations N] [--population P] [--seed S] [--body-mutation] [--training PATH] [--out FILE] [--replay FILE]`.

Note: `combine --json` prints `"genome": hex(&result.genome_hash)`, and `result.genome_hash` must equal `champion.hash()`. Confirm `Evaluation.genome_hash` is `genome.hash()` in `combine.rs`; if it differs, compare against the `combine` output for the champion share string instead.

- [ ] **Step 4: Run to verify it passes**

Run: `./scripts/cargo test -p bestiary-cli --test commands --locked train_command`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/bestiary-cli
git commit -m "bestiary train command"
```

---

### Task 7: Viewer training state and worker

**Files:**
- Create: `crates/bestiary-viewer/src/training.rs`
- Modify: `crates/bestiary-viewer/src/main.rs` (add `mod training;`, a `training: training::TrainingView` field on `View`, initialise it, `.insert_resource(training::Worker::new())`, and a system `training::poll` added alongside the existing feed polling system)
- Test: inline tests in `training.rs`

**Interfaces:**
- Consumes: `bestiary_sim::training::{Training, Task, TrainingSettings, GenerationReport}`
- Produces:
  - `pub enum Source { Preset(usize), Designer, Selected }`
  - `pub struct TrainingView { pub source: Source, pub task: Task, pub settings: TrainingSettings, pub seed: u64, pub run: u64, pub running: bool, pub history: Vec<Summary>, pub champions: Vec<(Genome, Replay)>, pub latest: Option<GenerationReport>, pub shown: Option<u32>, pub show_all: bool, pub time: f32, pub status: String }`
  - `pub struct Summary { pub generation: u32, pub best: f32, pub median: f32 }`
  - `impl TrainingView { pub fn new() -> Self; pub fn reset(&mut self); pub fn accept(&mut self, run: u64, report: GenerationReport) -> bool; pub fn slots(&self) -> Vec<(&Genome, &Replay, bool)> }`. `slots` returns the bodies to draw, with the bool meaning champion. When `shown` is `None` and `show_all` is set, it returns every individual of `latest`. Otherwise it returns only the champion of `shown` (or of the latest generation).
  - `pub enum Command { Start { run: u64, source: Genome, task: Task, settings: TrainingSettings, config: Config, seed: u64 }, Pause, Resume, Stop }`
  - `pub enum Event { Report(u64, GenerationReport), Failed(u64, String) }`
  - `#[derive(Resource)] pub struct Worker { pub tx: mpsc::Sender<Command>, pub rx: Mutex<mpsc::Receiver<Event>> }` and `impl Worker { pub fn new() -> Self }`

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    fn report(generation: u32, best: f32) -> GenerationReport {
        let c = Config::bundled();
        let g = bestiary_core::genome::Genome::starter(&c.genome);
        let replay = Replay { kind: bestiary_sim::combine::TrialKind::Sprint, dt: 1.0 / 240.0, frames: vec![] };
        GenerationReport { generation, best, median: best / 2.0, worst: 0.0, champion: g.clone(),
            champion_replay: replay.clone(), runs: vec![], genomes: vec![g.clone(), g], replays: vec![replay.clone(), replay] }
    }
    #[test]
    fn training_view_accepts_reports_only_from_the_current_run() {
        let mut v = TrainingView::new();
        v.run = 2;
        assert!(!v.accept(1, report(0, 1.0)), "stale run accepted");
        assert!(v.accept(2, report(0, 1.0)));
        assert!(v.accept(2, report(1, 1.5)));
        assert_eq!(v.history.iter().map(|s| s.best).collect::<Vec<_>>(), vec![1.0, 1.5]);
        assert_eq!(v.champions.len(), 2);
    }
    #[test]
    fn training_view_slots_show_generation_or_champion() {
        let mut v = TrainingView::new();
        v.accept(v.run, report(0, 1.0));
        v.show_all = true;
        assert_eq!(v.slots().len(), 2);
        assert!(v.slots()[0].2 && !v.slots()[1].2);
        v.shown = Some(0);
        assert_eq!(v.slots().len(), 1);
        v.reset();
        assert!(v.slots().is_empty() && v.history.is_empty());
    }
    #[test]
    fn worker_reports_generations_and_stops_on_pause() {
        let c = Config::bundled();
        let w = Worker::new();
        let settings = TrainingSettings { population: 2, ..TrainingSettings::bundled() };
        let source = bestiary_core::genome::Genome::starter(&c.genome);
        w.tx.send(Command::Start { run: 9, source, task: Task::Sprint, settings, config: c, seed: 1 }).unwrap();
        let rx = w.rx.lock().unwrap();
        let first = rx.recv_timeout(std::time::Duration::from_secs(120)).unwrap();
        assert!(matches!(first, Event::Report(9, ref r) if r.generation == 0));
        w.tx.send(Command::Pause).unwrap();
        // At most the in-flight generation completes after Pause.
        let mut after = 0;
        while rx.recv_timeout(std::time::Duration::from_secs(20)).is_ok() { after += 1; }
        assert!(after <= 1, "{after} reports after pause");
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `./scripts/cargo test -p bestiary-viewer --locked --bin bestiary-viewer training`
Expected: compile error.

- [ ] **Step 3: Implement `training.rs`**

```rust
//! Training panel state and its background worker. All physics runs in
//! `bestiary_sim::training`; this module only schedules it and keeps results.
use bestiary_core::genome::Genome;
use bestiary_sim::{combine::Replay, config::Config, training::{GenerationReport, Task, Training, TrainingSettings}};
use bevy::prelude::Resource;
use std::sync::{mpsc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source { Preset(usize), Designer, Selected }
#[derive(Clone, Debug, PartialEq)]
pub struct Summary { pub generation: u32, pub best: f32, pub median: f32 }

pub struct TrainingView {
    pub source: Source,
    pub task: Task,
    pub settings: TrainingSettings,
    pub seed: u64,
    pub run: u64,
    pub running: bool,
    pub history: Vec<Summary>,
    pub champions: Vec<(Genome, Replay)>,
    pub latest: Option<GenerationReport>,
    pub shown: Option<u32>,
    pub show_all: bool,
    pub time: f32,
    pub status: String,
}
impl TrainingView {
    pub fn new() -> Self {
        Self { source: Source::Preset(0), task: Task::Sprint, settings: TrainingSettings::bundled(), seed: 42,
               run: 0, running: false, history: vec![], champions: vec![], latest: None, shown: None,
               show_all: true, time: 0.0, status: String::new() }
    }
    /// Discards results; the next Start uses a new run id so late reports are ignored.
    pub fn reset(&mut self) {
        self.run += 1;
        self.running = false;
        self.history.clear();
        self.champions.clear();
        self.latest = None;
        self.shown = None;
        self.time = 0.0;
    }
    pub fn accept(&mut self, run: u64, report: GenerationReport) -> bool {
        if run != self.run { return false; }
        self.history.push(Summary { generation: report.generation, best: report.best, median: report.median });
        self.champions.push((report.champion.clone(), report.champion_replay.clone()));
        self.latest = Some(report);
        self.time = 0.0;
        true
    }
    pub fn slots(&self) -> Vec<(&Genome, &Replay, bool)> {
        match (self.shown, &self.latest) {
            (None, Some(latest)) if self.show_all => latest.genomes.iter().zip(&latest.replays)
                .enumerate().map(|(i, (g, r))| (g, r, i == 0)).collect(),
            (Some(n), _) => self.champions.get(n as usize).map(|(g, r)| vec![(g, r, true)]).unwrap_or_default(),
            (None, Some(_)) => self.champions.last().map(|(g, r)| vec![(g, r, true)]).unwrap_or_default(),
            (None, None) => vec![],
        }
    }
}
impl Default for TrainingView { fn default() -> Self { Self::new() } }

pub enum Command {
    Start { run: u64, source: Genome, task: Task, settings: TrainingSettings, config: Config, seed: u64 },
    Pause,
    Resume,
    Stop,
}
pub enum Event { Report(u64, GenerationReport), Failed(u64, String) }

#[derive(Resource)]
pub struct Worker { pub tx: mpsc::Sender<Command>, pub rx: Mutex<mpsc::Receiver<Event>> }
impl Worker {
    pub fn new() -> Self {
        let (tx, commands) = mpsc::channel::<Command>();
        let (events, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut current: Option<(u64, Training)> = None;
            let mut paused = false;
            loop {
                // Block when idle or paused; otherwise drain pending commands between generations.
                let next = if current.is_none() || paused { commands.recv().ok() } else { commands.try_recv().ok() };
                match next {
                    Some(Command::Start { run, source, task, settings, config, seed }) => {
                        paused = false;
                        current = match Training::new(source, task, settings, config, seed) {
                            Ok(t) => Some((run, t)),
                            Err(e) => { let _ = events.send(Event::Failed(run, e)); None }
                        };
                        continue;
                    }
                    Some(Command::Pause) => { paused = true; continue; }
                    Some(Command::Resume) => { paused = false; continue; }
                    Some(Command::Stop) => { current = None; continue; }
                    None if current.is_none() || paused => return, // channel closed
                    None => {}
                }
                if let Some((run, training)) = &mut current {
                    let event = match training.next_generation(true) {
                        Ok(r) => Event::Report(*run, r),
                        Err(e) => { let failed = Event::Failed(*run, e); current = None; failed }
                    };
                    if events.send(event).is_err() { return; }
                }
            }
        });
        Self { tx, rx: Mutex::new(rx) }
    }
}
```

Note: when `try_recv` returns `Empty` it maps to `None` and the loop runs a generation. When it returns `Disconnected`, the worker should exit. Replace `commands.try_recv().ok()` with a match that returns from the thread on `TryRecvError::Disconnected`.

In `main.rs`, add a system:

```rust
fn poll_training(mut view: ResMut<View>, worker: Res<training::Worker>) {
    let Ok(rx) = worker.rx.lock() else { return };
    while let Ok(event) = rx.try_recv() {
        match event {
            training::Event::Report(run, report) => {
                if view.training.accept(run, report) && view.tab == Tab::Training { view.dirty = true; }
            }
            training::Event::Failed(run, error) if run == view.training.run => {
                view.training.running = false;
                view.training.status = error;
            }
            training::Event::Failed(..) => {}
        }
    }
}
```

Register it in the same `add_systems(Update, ...)` tuple as the existing feed-polling system.

- [ ] **Step 4: Run to verify they pass**

Run: `./scripts/cargo test -p bestiary-viewer --locked --bin bestiary-viewer training`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/bestiary-viewer/src/training.rs crates/bestiary-viewer/src/main.rs
git commit -m "Viewer training state and background worker"
```

---

### Task 8: Training tab, arena rendering and panel

**Files:**
- Create: `crates/bestiary-viewer/src/ui_training.rs`
- Modify: `crates/bestiary-viewer/src/main.rs`. Add `Tab::Training`. Add `Some("training") => Tab::Training` to the `--panel` parser and the usage string. Add `mod ui_training;`. In `simulate_inspector`'s early return, leave the Training tab out of `is_body()`.
- Modify: `crates/bestiary-viewer/src/ui.rs`. Add `(Tab::Training, "Training")` to the tab list after the Designer entry, and `Tab::Training => crate::ui_training::panel(ctx, &mut view, &worker, &feed),` in the panel match. Add `worker: Res<training::Worker>` to that system's parameters.
- Modify: `crates/bestiary-viewer/src/scene.rs`. Replace the `inspector: bool` field on `BodyPart` and `MuscleVisual`, and the `inspector: bool` parameter of `draw_body`, with `source: PoseSource`. Add a Training branch to `rebuild`, pose selection in `animate`/`animate_tissue`, and camera targeting.
- Modify: `crates/bestiary-viewer/tests/render.rs` (add `"Training"` to the panel list)

**Interfaces:**
- Consumes: `TrainingView`, `Worker`, `Command`, `Source` (Task 7); `designer::Designer::new(genome, &config)`; `bestiary_daemon::Request::Release { genome, position, count }`; `Job::Release`
- Produces: `pub(crate) enum PoseSource { Inspector, WorldSprint, Training(usize) }`; `pub fn panel(ctx: &egui::Context, view: &mut View, worker: &training::Worker, feed: &Feed)`; `pub(crate) fn fitness_chart(ui: &mut egui::Ui, history: &[training::Summary])`

- [ ] **Step 1: Write the failing unit tests (in `ui_training.rs`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fitness_series_maps_generations_to_chart_space() {
        let h = vec![training::Summary { generation: 0, best: 0.0, median: 0.0 },
                     training::Summary { generation: 1, best: 2.0, median: 1.0 }];
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(100.0, 50.0));
        let (best, median) = series(&h, rect);
        assert_eq!(best, vec![egui::pos2(0.0, 50.0), egui::pos2(100.0, 0.0)]);
        assert_eq!(median[1], egui::pos2(100.0, 25.0));
        assert!(series(&[], rect).0.is_empty());
    }
    #[test]
    fn lane_offsets_are_distinct_and_centered() {
        let lanes: Vec<_> = (0..5).map(|i| lane_offset(i, 5)).collect();
        assert_eq!(lanes[2], bevy::prelude::Vec3::ZERO);
        assert!(lanes.windows(2).all(|w| (w[1].z - w[0].z - LANE_SPACING).abs() < 1e-5));
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `./scripts/cargo test -p bestiary-viewer --locked --bin bestiary-viewer ui_training`
Expected: compile error.

- [ ] **Step 3: Implement `ui_training.rs`**

```rust
use super::*;
use crate::training::{self, Command, Source};
use bestiary_sim::training::Task;

pub(crate) const LANE_SPACING: f32 = 1.6;
pub(crate) fn lane_offset(slot: usize, count: usize) -> Vec3 {
    Vec3::new(0.0, 0.0, (slot as f32 - (count.saturating_sub(1)) as f32 / 2.0) * LANE_SPACING)
}
pub(crate) fn series(h: &[training::Summary], rect: egui::Rect) -> (Vec<egui::Pos2>, Vec<egui::Pos2>) {
    if h.is_empty() { return (vec![], vec![]); }
    let top = h.iter().map(|s| s.best).fold(f32::MIN, f32::max).max(1e-6);
    let span = (h.len() - 1).max(1) as f32;
    let point = |i: usize, v: f32| egui::pos2(rect.left() + rect.width() * i as f32 / span,
                                               rect.bottom() - rect.height() * (v.max(0.0) / top));
    (h.iter().enumerate().map(|(i, s)| point(i, s.best)).collect(),
     h.iter().enumerate().map(|(i, s)| point(i, s.median)).collect())
}
pub(crate) fn fitness_chart(ui: &mut egui::Ui, history: &[training::Summary]) {
    let (response, painter) = ui.allocate_painter(egui::vec2(ui.available_width(), 160.0), egui::Sense::hover());
    painter.rect_filled(response.rect, 4.0, egui::Color32::from_rgb(9, 16, 21));
    let (best, median) = series(history, response.rect.shrink(8.0));
    painter.add(egui::Shape::line(median, egui::Stroke::new(1.5, egui::Color32::GRAY)));
    painter.add(egui::Shape::line(best, egui::Stroke::new(2.0, TEAL)));
    ui.small("Teal: best of generation · gray: median");
}
pub fn panel(ctx: &egui::Context, view: &mut View, worker: &training::Worker, feed: &Feed) {
    let config = view.snapshot.world.config.clone();
    let zoo = bestiary_core::genome::zoo(&config.genome);
    egui::SidePanel::right("training").min_width(320.0).show(ctx, |ui| {
        ui.heading("Training");
        let t = &mut view.training;
        ui.add_enabled_ui(!t.running, |ui| {
            egui::ComboBox::from_label("Body").selected_text(match t.source {
                Source::Preset(i) => zoo[i].0.to_string(),
                Source::Designer => "Designer draft".into(),
                Source::Selected => "Selected creature".into(),
            }).show_ui(ui, |ui| {
                for (i, (name, _)) in zoo.iter().enumerate() {
                    ui.selectable_value(&mut t.source, Source::Preset(i), *name);
                }
                ui.selectable_value(&mut t.source, Source::Designer, "Designer draft");
                ui.selectable_value(&mut t.source, Source::Selected, "Selected creature");
            });
            egui::ComboBox::from_label("Task").selected_text(t.task.name()).show_ui(ui, |ui| {
                for task in Task::ALL { ui.selectable_value(&mut t.task, task, task.name()); }
            });
            ui.add(egui::Slider::new(&mut t.settings.population, 2..=200).text("population"));
            ui.checkbox(&mut t.settings.body_mutation, "Allow body mutation");
        });
        ui.horizontal(|ui| {
            if !t.running && ui.button(if t.history.is_empty() { "Start" } else { "Resume" }).clicked() {
                if t.history.is_empty() {
                    let source = match t.source {
                        Source::Preset(i) => Some(zoo[i].1.clone()),
                        Source::Designer => Some(view.designer.genome.clone()),
                        Source::Selected => view.snapshot.world.genomes.get(&view.genome).map(|g| g.genome.clone()),
                    };
                    match source {
                        Some(source) => {
                            t.reset();
                            let _ = worker.tx.send(Command::Start { run: t.run, source, task: t.task,
                                settings: t.settings.clone(), config: config.clone(), seed: t.seed });
                            t.running = true;
                            t.status = "Evaluating generation 0…".into();
                        }
                        None => t.status = "Select a creature first".into(),
                    }
                } else {
                    let _ = worker.tx.send(Command::Resume);
                    t.running = true;
                }
            }
            if t.running && ui.button("Pause").clicked() {
                let _ = worker.tx.send(Command::Pause);
                t.running = false;
            }
            if ui.button("Reset").clicked() {
                let _ = worker.tx.send(Command::Stop);
                t.reset();
                view.dirty = true;
            }
        });
        if let Some(last) = t.history.last() {
            ui.label(format!("Generation {} · best {:.3} · median {:.3}", last.generation, last.best, last.median));
        }
        if !t.status.is_empty() { ui.small(&t.status); }
        fitness_chart(ui, &t.history);
        if !t.champions.is_empty() {
            let mut shown = t.shown.map_or(t.champions.len() as u32 - 1, |n| n);
            let latest = t.champions.len() as u32 - 1;
            if ui.add(egui::Slider::new(&mut shown, 0..=latest).text("generation")).changed() {
                t.shown = (shown != latest).then_some(shown);
                view.dirty = true;
            }
            if ui.checkbox(&mut t.show_all, "Show whole latest generation").changed() { view.dirty = true; }
            let champion = t.champions[shown as usize].0.clone();
            ui.horizontal(|ui| {
                if ui.button("Save genome").clicked() {
                    let path = std::path::Path::new("exports/training").join(format!("{}.genome", bestiary_core::hex(&champion.hash())));
                    t.status = std::fs::create_dir_all("exports/training")
                        .and_then(|_| std::fs::write(&path, champion.to_share()))
                        .map(|_| format!("Saved {}", path.display())).unwrap_or_else(|e| e.to_string());
                }
                if ui.button("Open in designer").clicked() {
                    view.designer = designer::Designer::new(champion.clone(), &config);
                    view.tab = Tab::Designer;
                    view.dirty = true;
                }
                if ui.add_enabled(view.snapshot.live, egui::Button::new("Release into world")).clicked() {
                    let command = bestiary_daemon::Request::Release { genome: champion.clone(),
                        position: view.designer.position, count: view.designer.population };
                    view.training.status = if feed.tx.send(Job::Release(command)).is_ok() { "Releasing…" } else { "World connection unavailable" }.into();
                }
            });
        }
    });
}
```

Make `TEAL` in `ui.rs` `pub(crate)` and import it here with `use crate::ui::TEAL;`. `view.training` is borrowed mutably as `t` across the closure. If the borrow checker rejects the later uses of `view.designer`/`view.dirty`, copy the needed values out first (`let live = view.snapshot.live;`, etc.) and set `view.dirty` after the panel closure returns, using a local `dirty` flag.

- [ ] **Step 4: Scene changes in `scene.rs`**

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PoseSource { Inspector, WorldSprint, Training(usize) }

fn source_frames<'a>(view: &'a View, inspector: &'a Inspector, source: PoseSource, hash: [u8; 32]) -> Option<&'a [Pose]> {
    match source {
        PoseSource::Inspector => inspector_poses(view, inspector),
        PoseSource::WorldSprint => view.replays.get(&(hash, TrialKind::Sprint))
            .and_then(|r| r.frames.get(playback::frame_index(view.world_play_time, r.dt, r.frames.len())))
            .map(Vec::as_slice),
        PoseSource::Training(slot) => view.training.slots().get(slot)
            .and_then(|(_, r, _)| r.frames.get(playback::clamped_frame_index(view.training.time, r.dt, r.frames.len())))
            .map(Vec::as_slice),
    }
}
```

- In `BodyPart` and `MuscleVisual`, rename `inspector: bool` to `source: PoseSource`. In `draw_body`, rename the parameter to `source: PoseSource`. Existing call sites pass `PoseSource::Inspector` where they passed `true`, and `PoseSource::WorldSprint` where they passed `false`.
- In `animate` and `animate_tissue`, replace the `if part.inspector { … } else { … }` frame selection with `source_frames(&view, &inspector, part.source, part.hash)`.
- `displayed_point(..., inspector: bool)` keeps its signature; pass `part.source != PoseSource::WorldSprint`, so training bodies stay in arena space.
- Muscle activation stays `muscle.source == PoseSource::Inspector && view.live_preview && …`.
- In `rebuild`, before the `if view.tab.is_body()` branch:

```rust
if view.tab == Tab::Training {
    let floor = palette.material(appearance.ground_color, &mut materials);
    commands.spawn((Mesh3d(palette.cube.clone()), MeshMaterial3d(floor),
        Transform::from_xyz(0.0, -0.1, 0.0).with_scale(Vec3::new(200.0, 0.2, 200.0)), Visual));
    let slots = view.training.slots();
    let count = slots.len();
    for (slot, (genome, _, champion)) in slots.into_iter().enumerate() {
        if let Ok(body) = genome.expand(&world.config.genome) {
            draw_body(&mut commands, &mut palette, &mut materials, &mut meshes, genome.hash(), &body, genome,
                crate::ui_training::lane_offset(slot, count), PoseSource::Training(slot),
                (!champion).then_some([0.42, 0.46, 0.48]), view.anatomy && champion, &appearance);
        }
    }
    return;
}
```

- In `camera`, add a branch before the `is_body()` branch. When `view.tab == Tab::Training`, target the champion slot's current root: `source_frames(&view, &inspector, PoseSource::Training(0), [0; 32]).and_then(|p| p.first())` plus `lane_offset(0, count)`, defaulting to `Vec3::new(0.0, 0.7, 0.0)`.
- Advance `view.training.time += time.delta_secs()` each frame when the tab is Training. Do it in the existing system that advances `view.play_time`. Wrap to 0 when it passes the champion replay's duration (`frames.len() as f32 * dt`).
- When switching to the Training tab in `ui.rs`'s tab loop, set `view.radius = 8.0`.

- [ ] **Step 5: Add `"Training"` to the panel list in `tests/render.rs`, and to the `--panel` match in `main.rs`**

- [ ] **Step 6: Run the unit tests, clippy and the GPU render check**

Run: `./scripts/cargo test -p bestiary-viewer --locked`
Expected: all pass.
Run: `./scripts/cargo clippy --workspace --all-targets --locked -- -D warnings`
Expected: clean.
Run: `./scripts/check-viewer-render`
Expected: passes, including the Training panel PNG.

- [ ] **Step 7: Manual look (human)**

Run: `./target/release/bestiary-viewer var/demo-zoo.db --panel training`, choose the swimmer and Swim, then Start. Expected: the chart grows one point per generation, the lanes animate, and Pause stops after the current generation.

- [ ] **Step 8: Commit**

```bash
git add crates/bestiary-viewer
git commit -m "Training tab: arena lanes, fitness chart, generation scrubber, actions"
```

---

### Task 9: Docs, gates and final commit

**Files:**
- Modify: `RUNBOOK.md` (new section "Train a body", after "Design, combine and release a creature")
- Modify: `TUNING.md` (a `config/training.ron` section with every field)
- Modify: `ASSUMPTIONS.md`, `PLAN.md`

- [ ] **Step 1: RUNBOOK section**

````markdown
## Train a body

Training evolves a body's motor brain for one physics task, generation by
generation, in the manner of Keiwan's Evolution. The body stays fixed unless
body mutation is enabled.

```sh
./target/release/bestiary train swimmer --task swim --generations 30 --out exports/swimmer.genome --replay exports/swimmer.rpl
./target/release/bestiary combine exports/swimmer.genome
```

In the viewer, open **Training** (or `--panel training`), choose a body and a
task, and press **Start**. Each generation's runs are drawn side by side, with
the champion in full colour; the chart shows best and median fitness. Use the
generation slider to replay any generation's champion, then **Save genome**,
**Open in designer** or **Release into world**. Settings are in
`config/training.ron`; override them with `bestiary train … --training PATH`.
Runs are deterministic for a given body, task, settings and `--seed`.
````

- [ ] **Step 2: TUNING section listing each `training.ron` field, its effect and its symptom.** Examples: population too small means a noisy best curve; weight_sigma too large means the best stalls while the median collapses; oscillator_sigma 0 means gait timing never changes.

- [ ] **Step 3: ASSUMPTIONS entries** covering: controller mutation is not an `Operator` (to keep the encoding stable); fitness definitions per task; elites carry their earlier fitness and are not re-evaluated; the measured improvement numbers from Task 5; the viewer ghosts by colour, not transparency.

- [ ] **Step 4: Full gates**

Run: `make check`
Expected: exit 0.
Run: `make bench`
Expected: combine < 2 s, world ≥ 10,000 ticks/min, viewer ≥ 60 fps. Record the numbers in `RUNBOOK.md`'s measured section, labelled with this host.

- [ ] **Step 5: Commit**

```bash
git add RUNBOOK.md TUNING.md ASSUMPTIONS.md PLAN.md
git commit -m "Training mode: docs, tuning and validation"
```
