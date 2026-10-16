//! Training acceptance: measured improvement on two bodies the starter zoo
//! cannot drive well. Margins were set from a measured run (see ASSUMPTIONS.md).
use bestiary_core::genome::{zoo, Genome};
use bestiary_sim::{
    combine::{trial, TrialKind},
    config::Config,
    training::{Task, Training, TrainingSettings},
};

const MARGIN_SPRINT: f32 = 0.15;
const MARGIN_SWIM: f32 = 0.11;

fn preset(c: &Config, name: &str) -> Genome {
    zoo(&c.genome)
        .into_iter()
        .find(|(n, _)| *n == name)
        .unwrap()
        .1
}
/// Best fitness after `generations` generations at a fixed seed.
fn trained_best(source: Genome, task: Task, generations: u32) -> f32 {
    let c = Config::bundled();
    let settings = TrainingSettings {
        population: 20,
        ..TrainingSettings::bundled()
    };
    let mut t = Training::new(source, task, settings, c, 42).unwrap();
    let mut best = f32::NEG_INFINITY;
    for _ in 0..generations {
        let r = t.next_generation(true).unwrap();
        println!(
            "{task:?} generation {} best {} median {}",
            r.generation, r.best, r.median
        );
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
            for conn in &mut n.inputs {
                conn.weight = 0.0;
            }
        }
    }
    let source = trial(&g, &c, TrialKind::Sprint, 0.0).unwrap().speed;
    println!("zeroed grazer source speed {source}");
    let best = trained_best(g, Task::Sprint, 15);
    assert!(
        best >= source + MARGIN_SPRINT,
        "best {best} vs source {source}"
    );
}

#[test]
fn training_improves_swimmer_swim() {
    let c = Config::bundled();
    let g = preset(&c, "swimmer");
    let r = trial(&g, &c, TrialKind::Swim, 0.0).unwrap();
    let source = r.distance / c.combine.trial_seconds;
    println!("swimmer source swim {source}");
    let best = trained_best(g, Task::Swim, 15);
    assert!(
        best >= source + MARGIN_SWIM,
        "best {best} vs source {source}"
    );
}
