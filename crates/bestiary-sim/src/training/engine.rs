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
    Operator::Parameter,
    Operator::AddBone,
    Operator::RemoveBone,
    Operator::AddJoint,
    Operator::RemoveJoint,
    Operator::JointType,
    Operator::Recursion,
    Operator::Reflection,
    Operator::DuplicateSubtree,
    Operator::PruneSubtree,
    Operator::AddMuscle,
    Operator::RemoveMuscle,
    Operator::MoveAttachment,
    Operator::MuscleArea,
    Operator::MuscleSpan,
    Operator::AddSensor,
    Operator::RemoveSensor,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Task {
    Sprint,
    Swim,
    Turn,
    Slope,
    Rough,
    Strike,
}
impl Task {
    pub const ALL: [Task; 6] = [
        Task::Sprint,
        Task::Swim,
        Task::Turn,
        Task::Slope,
        Task::Rough,
        Task::Strike,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Task::Sprint => "sprint",
            Task::Swim => "swim",
            Task::Turn => "turn",
            Task::Slope => "slope",
            Task::Rough => "rough",
            Task::Strike => "strike",
        }
    }
    pub fn parse(s: &str) -> Option<Task> {
        Task::ALL.into_iter().find(|t| t.name() == s)
    }
    pub fn kind(self) -> TrialKind {
        match self {
            Task::Sprint => TrialKind::Sprint,
            Task::Swim => TrialKind::Swim,
            Task::Turn => TrialKind::Turn,
            Task::Slope => TrialKind::Slope,
            Task::Rough => TrialKind::Rough,
            Task::Strike => TrialKind::Strike,
        }
    }
    /// Fitness uses the same measurements as the combine's trait extraction.
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
pub struct RunSummary {
    pub hash: [u8; 32],
    pub fitness: f32,
    pub exploit: bool,
}

/// One evaluated generation. `runs[i]`, `genomes[i]` and `replays[i]` describe
/// the same individual, best first.
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

/// Indices best-first: clean finite fitness descending, then exploit-suspect or
/// non-finite runs; ties by ascending genome hash.
pub fn rank(runs: &[RunSummary]) -> Vec<usize> {
    let key = |r: &RunSummary| {
        if r.exploit || !r.fitness.is_finite() {
            f32::NEG_INFINITY
        } else {
            r.fitness
        }
    };
    let mut order: Vec<usize> = (0..runs.len()).collect();
    order.sort_by(|&a, &b| {
        key(&runs[b])
            .total_cmp(&key(&runs[a]))
            .then(runs[a].hash.cmp(&runs[b].hash))
    });
    order
}

/// How many leading runs of a ranked list may become elites: at most `limit`,
/// and never an exploit-suspect or non-finite run.
pub fn elite_count(ranked: &[RunSummary], limit: usize) -> usize {
    ranked
        .iter()
        .take_while(|r| !r.exploit && r.fitness.is_finite())
        .count()
        .min(limit)
}

#[derive(Clone)]
struct Individual {
    genome: Genome,
    run: RunSummary,
    replay: Replay,
}

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
    pub fn new(
        source: Genome,
        task: Task,
        settings: TrainingSettings,
        config: Config,
        seed: u64,
    ) -> Result<Self, String> {
        settings.validate()?;
        source.validate(&config.genome)?;
        source.expand(&config.genome)?;
        crate::genome::Creature::new(source.clone(), &config)?;
        let mut t = Training {
            task,
            settings,
            config,
            seed,
            generation: 0,
            pending: vec![],
            elites: vec![],
        };
        t.pending = std::iter::once(source.clone())
            .chain((1..t.settings.population).map(|k| t.child(&source, k as u64)))
            .collect();
        Ok(t)
    }
    pub fn generation(&self) -> u32 {
        self.generation
    }

    fn child(&self, parent: &Genome, k: u64) -> Genome {
        let tick = self.generation as u64;
        let mut g = parent.mutate_controller(&self.settings.controller(), self.seed, k, tick);
        if self.settings.body_mutation {
            let mut r = bestiary_core::rng(self.seed, TRAINING_SUBSYSTEM, k, tick);
            let op = BODY_OPERATORS[r.gen_range(0..BODY_OPERATORS.len())];
            g = g.mutate(op, self.seed ^ k, tick, &self.config.genome).0;
        }
        g
    }

    fn evaluate(&self, g: &Genome) -> Individual {
        let slope = if self.task == Task::Slope {
            self.settings.slope_angle
        } else {
            0.0
        };
        let hash = g.hash();
        let empty = || Replay {
            kind: self.task.kind(),
            dt: crate::PHYSICS_DT,
            frames: vec![],
        };
        match trial_recorded(g, &self.config, self.task.kind(), slope, true) {
            Ok(r) => Individual {
                genome: g.clone(),
                run: RunSummary {
                    hash,
                    fitness: self.task.fitness(&r, &self.config),
                    exploit: r.energy.exploit_suspect,
                },
                replay: r.replay.unwrap_or_else(empty),
            },
            Err(_) => Individual {
                genome: g.clone(),
                run: RunSummary {
                    hash,
                    fitness: f32::NEG_INFINITY,
                    exploit: false,
                },
                replay: empty(),
            },
        }
    }

    /// Evaluates the pending population (elites keep their earlier result), ranks
    /// everyone, reports, and breeds the next generation from the elites.
    pub fn next_generation(&mut self, parallel: bool) -> Result<GenerationReport, String> {
        let fresh: Vec<Individual> = if parallel {
            self.pending.par_iter().map(|g| self.evaluate(g)).collect()
        } else {
            self.pending.iter().map(|g| self.evaluate(g)).collect()
        };
        let all: Vec<Individual> = self.elites.drain(..).chain(fresh).collect();
        let runs: Vec<RunSummary> = all.iter().map(|i| i.run.clone()).collect();
        let all: Vec<Individual> = rank(&runs).into_iter().map(|i| all[i].clone()).collect();
        let champion = all.first().ok_or("empty population")?;
        if !champion.run.fitness.is_finite() || champion.run.exploit {
            return Err("no individual completed a clean trial".into());
        }
        let finite: Vec<f32> = all
            .iter()
            .filter(|i| !i.run.exploit && i.run.fitness.is_finite())
            .map(|i| i.run.fitness)
            .collect();
        let report = GenerationReport {
            generation: self.generation,
            best: finite[0],
            median: finite[finite.len() / 2],
            worst: finite[finite.len() - 1],
            champion: champion.genome.clone(),
            champion_replay: champion.replay.clone(),
            runs: all.iter().map(|i| i.run.clone()).collect(),
            genomes: all.iter().map(|i| i.genome.clone()).collect(),
            replays: all.iter().map(|i| i.replay.clone()).collect(),
        };
        // Only clean, finite runs may become parents; the champion check above
        // guarantees at least one.
        let elites = elite_count(&report.runs, self.settings.elites());
        self.elites = all.into_iter().take(elites).collect();
        self.generation += 1;
        self.pending = (0..(self.settings.population - elites) as u64)
            .map(|k| self.child(&self.elites[k as usize % elites].genome, k))
            .collect();
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn small() -> TrainingSettings {
        TrainingSettings {
            population: 6,
            ..TrainingSettings::bundled()
        }
    }
    fn grazer(c: &Config) -> Genome {
        bestiary_core::genome::zoo(&c.genome)
            .into_iter()
            .find(|(n, _)| *n == "grazer")
            .unwrap()
            .1
    }
    fn run(seed: u64, parallel: bool, generations: u32) -> Vec<GenerationReport> {
        let c = Config::bundled();
        let mut t = Training::new(grazer(&c), Task::Sprint, small(), c, seed).unwrap();
        (0..generations)
            .map(|_| t.next_generation(parallel).unwrap())
            .collect()
    }
    #[test]
    fn training_is_deterministic_and_serial_equals_parallel() {
        let a = run(3, true, 3);
        assert_eq!(a, run(3, true, 3));
        assert_eq!(a, run(3, false, 3));
        // The hand-tuned source can stay champion; the mutants are what the seed changes.
        assert_ne!(a[2].genomes, run(4, true, 3)[2].genomes);
    }
    #[test]
    fn training_best_fitness_never_decreases() {
        let reports = run(5, true, 4);
        for pair in reports.windows(2) {
            assert!(
                pair[1].best >= pair[0].best,
                "{} < {}",
                pair[1].best,
                pair[0].best
            );
        }
        assert_eq!(
            reports.iter().map(|r| r.generation).collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
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
        let s = |h: u8, fitness: f32, exploit: bool| RunSummary {
            hash: [h; 32],
            fitness,
            exploit,
        };
        let runs = vec![
            s(9, 1.0, false),
            s(1, 5.0, true),
            s(2, f32::NAN, false),
            s(4, 1.0, false),
            s(3, 0.5, false),
        ];
        // Equal fitness 1.0: hash 4 before hash 9. Exploit (raw 5.0) and NaN rank last, by hash.
        assert_eq!(rank(&runs), vec![3, 0, 4, 1, 2]);
        let still = vec![s(7, 0.0, false), s(2, 0.0, false), s(5, 0.0, false)];
        assert_eq!(rank(&still), vec![1, 2, 0]);
    }
    #[test]
    fn elites_exclude_exploit_and_failed_runs() {
        let s = |fitness: f32, exploit: bool| RunSummary {
            hash: [0; 32],
            fitness,
            exploit,
        };
        // Ranked order: two clean runs, then an exploit and a failed run.
        let ranked = vec![
            s(2.0, false),
            s(1.0, false),
            s(9.0, true),
            s(f32::NEG_INFINITY, false),
        ];
        assert_eq!(elite_count(&ranked, 10), 2);
        assert_eq!(elite_count(&ranked, 1), 1);
        assert_eq!(elite_count(&ranked[2..], 3), 0);
    }
    #[test]
    fn motionless_body_still_reports_a_deterministic_champion() {
        let c = Config::bundled();
        let mut still = grazer(&c);
        for bone in &mut still.bones {
            for n in &mut bone.motor.neurons {
                n.bias = -16.0;
                for conn in &mut n.inputs {
                    conn.weight = 0.0;
                }
            }
        }
        let frozen = TrainingSettings {
            population: 4,
            weight_rate: 0.0,
            weight_sigma: 0.0,
            oscillator_sigma: 0.0,
            activation_rate: 0.0,
            ..TrainingSettings::bundled()
        };
        let mut t = Training::new(still.clone(), Task::Sprint, frozen, c, 1).unwrap();
        let first = t.next_generation(true).unwrap();
        let second = t.next_generation(true).unwrap();
        assert_eq!(first.champion, still);
        assert_eq!(second.best, first.best);
        assert!(first.runs.windows(2).all(|w| w[0].fitness == w[1].fitness));
    }
    #[test]
    fn body_mutation_mode_completes_and_reports_valid_champions() {
        let c = Config::bundled();
        let settings = TrainingSettings {
            body_mutation: true,
            ..small()
        };
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
            let settings = TrainingSettings {
                population: 2,
                ..TrainingSettings::bundled()
            };
            let mut t = Training::new(grazer(&c), task, settings, c.clone(), 2).unwrap();
            let report = t.next_generation(true).unwrap();
            assert_eq!(report.replays.len(), 2);
            assert_eq!(report.champion_replay.kind, task.kind());
        }
    }
}
