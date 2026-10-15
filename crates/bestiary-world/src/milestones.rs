//! Conservative observational milestones. These never alter actions or physics.
use super::*;
use bestiary_core::genome::{Body, Shape};
use bestiary_sim::{
    combine::{Evaluation, TrialKind},
    physics::Pose,
};

/// Evidence from one real decision, retained only until that decision expires.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PheromoneGuidance {
    pub victim: u64,
    pub decision_tick: u64,
    pub channels: u8,
    pub requires_movement: bool,
}

fn output(values: &[f32], index: usize) -> f32 {
    values.get(index).copied().unwrap_or(0.0)
}
fn pursues(values: &[f32]) -> bool {
    output(values, 4) > output(values, 3)
        && output(values, 5) <= output(values, 4)
        && output(values, 2) > 0.0
}
fn attacks(values: &[f32]) -> bool {
    output(values, 6) > output(values, 3).max(output(values, 8))
}

/// Evaluate exactly the same pre-decision recurrent state with only the eight
/// pheromone gradients removed. The cloned plastic weights are discarded.
pub(crate) fn without_pheromones(
    brain: &Brain,
    prior: &BrainState,
    inputs: &[f32],
    tick: u64,
) -> Option<(Vec<f32>, u8)> {
    let mut channels = 0;
    for channel in 0..4 {
        if inputs[5 + channel] != 0.0 || inputs[16 + channel] != 0.0 {
            channels |= 1 << channel;
        }
    }
    if channels == 0 || !brain.neurons.iter().zip(&prior.weights).any(|(neuron, weights)| {
        neuron.inputs.iter().zip(weights).any(|(connection, weight)| {
            matches!(connection.input, Input::Sensor(i) if ((5..9).contains(&i) || (16..20).contains(&i)) && inputs[i] != 0.0 && *weight != 0.0)
        })
    }) {
        return None;
    }
    let mut neutral_inputs = inputs.to_vec();
    neutral_inputs[5..9].fill(0.0);
    neutral_inputs[16..20].fill(0.0);
    let mut neutral = prior.clone();
    brain.step(&mut neutral, &neutral_inputs, &[], &[], tick as f32);
    Some((neutral.values, channels))
}

pub(crate) fn guidance(
    actual: &[f32],
    neutral: &[f32],
    channels: u8,
    victim: u64,
    tick: u64,
) -> Option<PheromoneGuidance> {
    let attack_changed = attacks(actual) && !attacks(neutral);
    let pursuit_changed = pursues(actual) && !pursues(neutral);
    (attack_changed || pursuit_changed).then_some(PheromoneGuidance {
        victim,
        decision_tick: tick,
        channels,
        requires_movement: !attack_changed,
    })
}

/// Exact lower support of the physical boxes/capsules in a recorded pose. The
/// sprint arena has a horizontal floor at y=0; cosmetic anatomy is irrelevant.
fn clearance(body: &Body, frame: &[Pose]) -> Option<f32> {
    if body.parts.is_empty() || frame.len() != body.parts.len() {
        return None;
    }
    let mut lowest = f32::INFINITY;
    for (part, pose) in body.parts.iter().zip(frame) {
        if pose
            .translation
            .iter()
            .chain(&pose.rotation)
            .any(|v| !v.is_finite())
        {
            return None;
        }
        let [x, y, z, w] = pose.rotation;
        let norm = x * x + y * y + z * z + w * w;
        if !norm.is_finite() || norm <= 0.0 {
            return None;
        }
        let row = [
            2.0 * (x * y + z * w) / norm,
            1.0 - 2.0 * (x * x + z * z) / norm,
            2.0 * (y * z - x * w) / norm,
        ];
        let half_height = match part.shape {
            Shape::Box => row
                .iter()
                .zip(part.size)
                .map(|(r, size)| r.abs() * size * 0.5)
                .sum(),
            Shape::Capsule => {
                let radius = part.size[0].min(part.size[2]) * 0.5;
                radius + (part.size[1] * 0.5 - radius).max(0.0) * row[1].abs()
            }
        };
        lowest = lowest.min(pose.translation[1] - half_height);
    }
    Some(lowest)
}

/// Two completed takeoff/landing cycles establish repetition, not flight.
/// A landing must actually intersect the floor in the recording; every part
/// must clear the existing stand-clearance threshold for at least two samples.
/// The recording starts after the combine's settling interval.
pub(crate) fn airborne_gait(
    genome: &Genome,
    evaluation: &Evaluation,
    config: &Config,
) -> Option<(u32, f32)> {
    if evaluation.exploit_suspect || evaluation.viability <= 0.0 || evaluation.traits.speed <= 0.0 {
        return None;
    }
    let trial = evaluation
        .trials
        .iter()
        .find(|trial| trial.kind == TrialKind::Sprint)?;
    if trial.skipped || trial.energy.active_work <= 0.0 {
        return None;
    }
    let replay = trial.replay.as_ref()?;
    if replay.kind != TrialKind::Sprint || replay.dt <= 0.0 || !replay.dt.is_finite() {
        return None;
    }
    let body = genome.expand(&config.genome).ok()?;
    let mut grounded = false;
    let mut clear_samples = 0;
    let mut qualified = false;
    let mut cycles = 0;
    let mut maximum = 0.0f32;
    for frame in &replay.frames {
        let lower = clearance(&body, frame)?;
        if lower <= 0.0 {
            if grounded && qualified {
                cycles += 1;
            }
            grounded = true;
            clear_samples = 0;
            qualified = false;
        } else if grounded && lower >= config.combine.root_clearance {
            clear_samples += 1;
            qualified |= clear_samples >= 2;
            maximum = maximum.max(lower);
        } else {
            clear_samples = 0;
        }
    }
    (cycles >= 2).then_some((cycles, maximum))
}

impl World {
    pub(crate) fn record_body_milestones(&mut self, genome: &Genome, evaluation: &Evaluation) {
        let hash = genome.hash();
        if !evaluation.exploit_suspect
            && evaluation.viability > 0.0
            && !self.first_swimmer
            && evaluation.traits.swim_speed > 0.0
        {
            self.first_swimmer = true;
            self.ecological_events
                .push(EcologyEvent::FirstSwimmer { genome: hash });
        }
        if !self.first_flyer_adjacent_gait {
            if let Some((cycles, max_clearance)) = airborne_gait(genome, evaluation, &self.config) {
                self.first_flyer_adjacent_gait = true;
                self.ecological_events
                    .push(EcologyEvent::FirstFlyerAdjacentGait {
                        genome: hash,
                        cycles,
                        max_clearance,
                    });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bestiary_sim::combine::{Replay, Traits, TrialResult};

    fn recorded_gait(levels: &[f32]) -> (Genome, Config, Evaluation) {
        let config = Config::bundled();
        let genome = Genome::starter(&config.genome);
        let body = genome.expand(&config.genome).unwrap();
        let mut resting: Vec<_> = body
            .parts
            .iter()
            .map(|part| Pose {
                translation: part.pose.translation.vector.into(),
                rotation: part.pose.rotation.coords.into(),
            })
            .collect();
        let base = clearance(&body, &resting).unwrap();
        for pose in &mut resting {
            pose.translation[1] -= base;
        }
        let frames = levels
            .iter()
            .map(|level| {
                resting
                    .iter()
                    .cloned()
                    .map(|mut pose| {
                        pose.translation[1] += level;
                        pose
                    })
                    .collect()
            })
            .collect();
        let evaluation = Evaluation {
            genome_hash: genome.hash(),
            cache_key: [0; 32],
            viability: 1.0,
            exploit_suspect: false,
            traits: Traits {
                speed: 1.0,
                ..Default::default()
            },
            trials: vec![TrialResult {
                kind: TrialKind::Sprint,
                skipped: false,
                speed: 1.0,
                acceleration: 0.0,
                distance: 1.0,
                progress: 1.0,
                turn_rate: 0.0,
                reach: 1.0,
                min_root_height: 0.1,
                strike_impulse: 0.0,
                gait_frequency: 1.0,
                energy: bestiary_sim::physics::EnergyAudit {
                    active_work: 1.0,
                    ..Default::default()
                },
                replay: Some(Replay {
                    kind: TrialKind::Sprint,
                    dt: bestiary_sim::PHYSICS_DT,
                    frames,
                }),
            }],
        };
        (genome, config, evaluation)
    }

    #[test]
    fn first_flyer_adjacent_gait_requires_two_complete_recorded_airborne_cycles() {
        let (genome, config, evaluation) =
            recorded_gait(&[-0.001, 0.2, 0.2, -0.001, 0.2, 0.2, -0.001]);
        assert_eq!(airborne_gait(&genome, &evaluation, &config).unwrap().0, 2);
        for levels in [
            vec![0.2, 0.2, -0.001],                 // Spawn/fall without a takeoff.
            vec![-0.001, 0.2, 0.2, -0.001],         // One hop is not a repeated gait.
            vec![-0.001, 0.2, -0.001, 0.2, -0.001], // One-frame numerical crossings.
            vec![-0.001, 0.001, 0.001, -0.001, 0.001, 0.001, -0.001],
            vec![-0.001, 0.2, 0.2, -0.001, 0.2, 0.2], // No second landing.
        ] {
            let (g, c, e) = recorded_gait(&levels);
            assert!(airborne_gait(&g, &e, &c).is_none(), "{levels:?}");
        }
        let mut unsafe_body = evaluation.clone();
        unsafe_body.exploit_suspect = true;
        assert!(airborne_gait(&genome, &unsafe_body, &config).is_none());
        let mut passive = evaluation.clone();
        passive.trials[0].energy.active_work = 0.0;
        assert!(airborne_gait(&genome, &passive, &config).is_none());
        let mut grounded = evaluation.clone();
        let replay = grounded.trials[0].replay.as_mut().unwrap();
        let rest = replay.frames[0].clone();
        for frame in &mut replay.frames {
            frame[1..].clone_from_slice(&rest[1..]);
        }
        assert!(
            airborne_gait(&genome, &grounded, &config).is_none(),
            "root bobbing with grounded feet is not airborne"
        );
        let mut malformed = evaluation.clone();
        malformed.trials[0].replay.as_mut().unwrap().frames[0].clear();
        assert!(airborne_gait(&genome, &malformed, &config).is_none());
    }

    #[test]
    fn airborne_evidence_uses_oriented_physical_boxes_and_capsules() {
        let (genome, config, _) = recorded_gait(&[]);
        let mut body = genome.expand(&config.genome).unwrap();
        body.parts.truncate(1);
        let s = libm::sqrtf(0.5);
        let pose = Pose {
            translation: [0.0, 1.0, 0.0],
            rotation: [0.0, 0.0, s, s],
        };
        body.parts[0].shape = Shape::Box;
        body.parts[0].size = [2.0, 4.0, 6.0];
        assert!(clearance(&body, std::slice::from_ref(&pose)).unwrap().abs() < 1e-6);
        body.parts[0].shape = Shape::Capsule;
        body.parts[0].size = [2.0, 4.0, 2.0];
        assert!(clearance(&body, &[pose]).unwrap().abs() < 1e-6);
    }

    #[test]
    fn first_airborne_gait_is_persisted_once_and_rejected_after_restart() {
        let (genome, config, evaluation) =
            recorded_gait(&[-0.001, 0.2, 0.2, -0.001, 0.2, 0.2, -0.001]);
        let mut world = World::new(12, config, 0).unwrap();
        world.ecological_events.clear();
        world.first_flyer_adjacent_gait = false;
        world.record_body_milestones(&genome, &evaluation);
        assert!(world.first_flyer_adjacent_gait);
        assert!(matches!(
            world.ecological_events.last(),
            Some(EcologyEvent::FirstFlyerAdjacentGait { cycles: 2, .. })
        ));
        let mut restored: World =
            postcard::from_bytes(&postcard::to_allocvec(&world).unwrap()).unwrap();
        restored.ecological_events.clear();
        restored.record_body_milestones(&genome, &evaluation);
        assert!(restored.ecological_events.is_empty());
    }

    #[test]
    fn pheromone_counterfactual_keeps_recurrence_plasticity_and_other_inputs_identical() {
        let mut brain = forager_brain();
        brain.neurons[6].inputs = vec![
            Connection {
                input: Input::Sensor(16),
                weight: 1.0,
                learning_rate: 0.2,
                decay: 0.1,
            },
            Connection {
                input: Input::Local(3),
                weight: 1.0,
                learning_rate: 0.0,
                decay: 0.0,
            },
        ];
        let mut prior = brain.state();
        prior.values[3] = 0.7;
        let original = prior.clone();
        let mut inputs = vec![0.0; 20];
        inputs[16] = 1.0;
        inputs[0] = 0.4;
        let (neutral, mask) = without_pheromones(&brain, &prior, &inputs, 17).unwrap();
        assert_eq!(prior, original);
        assert_eq!(mask, 1);
        let mut expected = prior.clone();
        inputs[16] = 0.0;
        brain.step(&mut expected, &inputs, &[], &[], 17.0);
        assert_eq!(neutral, expected.values);
        assert!(without_pheromones(&brain, &prior, &inputs, 17).is_none());
        inputs[16] = 1.0;
        brain.neurons[6].inputs.remove(0);
        assert!(without_pheromones(&brain, &brain.state(), &inputs, 17).is_none());
    }

    #[test]
    fn guidance_requires_a_changed_action_not_presence_of_a_pheromone_trail() {
        let mut actual = vec![0.0; 13];
        actual[3] = 0.5;
        actual[8] = 0.5;
        let neutral = actual.clone();
        assert!(guidance(&actual, &neutral, 1, 4, 7).is_none());
        actual[6] = 0.6;
        let evidence = guidance(&actual, &neutral, 1, 4, 7).unwrap();
        assert!(!evidence.requires_movement);
        actual[6] = 0.0;
        actual[4] = 0.8;
        actual[2] = 1.0;
        assert!(
            guidance(&actual, &neutral, 1, 4, 7)
                .unwrap()
                .requires_movement
        );
        actual[5] = 0.9;
        assert!(
            guidance(&actual, &neutral, 1, 4, 7).is_none(),
            "fleeing wins over pursuit"
        );
    }
}
