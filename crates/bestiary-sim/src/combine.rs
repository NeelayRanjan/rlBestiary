//! Public deterministic evaluation API. Parallelism occurs only across genomes.
use crate::{
    config::Config,
    genome::Creature,
    physics::{EnergyAudit, Fluid, Pose},
    PHYSICS_DT,
};
use bestiary_core::genome::Genome;
use nalgebra::Quaternion;
use rapier3d::prelude::*;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Full box dimensions of the strike trial's recorded target, in metres.
pub const STRIKE_TARGET_SIZE: [f32; 3] = [0.2, 0.3, 0.6];

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrialKind {
    Stand,
    Sprint,
    Turn,
    Rough,
    Slope,
    Swim,
    Strike,
    Carry,
    Reach,
}
impl TrialKind {
    pub const ALL: [Self; 9] = [
        Self::Stand,
        Self::Sprint,
        Self::Turn,
        Self::Rough,
        Self::Slope,
        Self::Swim,
        Self::Strike,
        Self::Carry,
        Self::Reach,
    ];
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Traits {
    pub speed: f32,
    pub acceleration: f32,
    pub energy_per_meter: f32,
    pub turn_rate: f32,
    pub rough_speed: f32,
    pub slope: f32,
    pub swim_speed: f32,
    pub strike_impulse: f32,
    pub carry_mass: f32,
    pub reach: f32,
    pub volume: f32,
    pub mass: f32,
    pub parts: usize,
    pub limbs: usize,
    pub symmetry: f32,
    pub gait_frequency: f32,
    pub standing_cost: f32,
    pub muscle_area: f32,
}
impl Traits {
    pub fn values(&self) -> Vec<f32> {
        vec![
            self.speed,
            self.acceleration,
            self.energy_per_meter,
            self.turn_rate,
            self.rough_speed,
            self.slope,
            self.swim_speed,
            self.strike_impulse,
            self.carry_mass,
            self.reach,
            self.volume,
            self.mass,
            self.parts as f32,
            self.limbs as f32,
            self.symmetry,
            self.gait_frequency,
            self.standing_cost,
            self.muscle_area,
        ]
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Replay {
    pub kind: TrialKind,
    pub dt: f32,
    pub frames: Vec<Vec<Pose>>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TrialResult {
    pub kind: TrialKind,
    pub skipped: bool,
    pub speed: f32,
    pub acceleration: f32,
    pub distance: f32,
    pub progress: f32,
    pub turn_rate: f32,
    pub reach: f32,
    /// Minimum clearance of the root collider's surface above the ground plane.
    pub min_root_height: f32,
    pub strike_impulse: f32,
    pub gait_frequency: f32,
    pub energy: EnergyAudit,
    pub replay: Option<Replay>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Evaluation {
    pub genome_hash: [u8; 32],
    pub cache_key: [u8; 32],
    pub traits: Traits,
    pub viability: f32,
    pub exploit_suspect: bool,
    pub trials: Vec<TrialResult>,
}
#[derive(Default)]
pub struct Combine {
    pub exploit_suspects: BTreeSet<[u8; 32]>,
    cache: BTreeMap<[u8; 32], Evaluation>,
}
impl Combine {
    pub fn evaluate(&mut self, g: &Genome, c: &Config) -> Result<Evaluation, String> {
        let key = cache_key(g, c);
        if let Some(v) = self.cache.get(&key) {
            return Ok(v.clone());
        }
        let v = evaluate(g, c)?;
        if v.exploit_suspect {
            self.exploit_suspects.insert(v.genome_hash);
        }
        self.cache.insert(key, v.clone());
        Ok(v)
    }
    pub fn evaluate_many(
        &mut self,
        genomes: &[Genome],
        config: &Config,
        parallel: bool,
    ) -> Result<Vec<Evaluation>, String> {
        let mut jobs: Vec<_> = genomes.iter().collect();
        jobs.sort_by_key(|g| g.hash());
        let results: Vec<_> = if parallel {
            jobs.par_iter().map(|g| evaluate(g, config)).collect()
        } else {
            jobs.iter().map(|g| evaluate(g, config)).collect()
        };
        let mut ordered = vec![];
        for result in results {
            let result = result?;
            if result.exploit_suspect {
                self.exploit_suspects.insert(result.genome_hash);
            }
            self.cache.insert(result.cache_key, result.clone());
            ordered.push(result);
        }
        Ok(ordered)
    }
}
fn cache_key(g: &Genome, c: &Config) -> [u8; 32] {
    bestiary_core::digest(&(crate::SIMULATION_REVISION, g.hash(), c)).expect("config serializes")
}
fn locomotion(kind: TrialKind) -> bool {
    matches!(
        kind,
        TrialKind::Sprint
            | TrialKind::Turn
            | TrialKind::Rough
            | TrialKind::Slope
            | TrialKind::Swim
            | TrialKind::Carry
    )
}
fn center_of_mass(creature: &Creature) -> Point<Real> {
    let (weighted, mass) = creature
        .arena
        .parts
        .iter()
        .take(creature.body.parts.len())
        .fold((Vector::zeros(), 0.0), |(weighted, mass), h| {
            let body = &creature.arena.bodies[*h];
            (
                weighted + body.center_of_mass().coords * body.mass(),
                mass + body.mass(),
            )
        });
    Point::from(weighted / mass)
}
fn root_clearance(creature: &Creature, ground: ColliderHandle) -> f32 {
    if creature.arena.touches_terrain(creature.arena.parts[0]) {
        return 0.0;
    }
    let ground = &creature.arena.colliders[ground];
    let ground_top = ground
        .shape()
        .as_cuboid()
        .expect("arena ground is a box")
        .half_extents
        .y;
    let ground_inverse = ground.position().inverse();
    creature.arena.bodies[creature.arena.parts[0]]
        .colliders()
        .iter()
        .map(|handle| {
            let collider = &creature.arena.colliders[*handle];
            collider
                .shape()
                .compute_aabb(&(ground_inverse * collider.position()))
                .mins
                .y
                - ground_top
        })
        .fold(f32::INFINITY, f32::min)
}
pub fn trial(g: &Genome, c: &Config, kind: TrialKind, slope: f32) -> Result<TrialResult, String> {
    let mut creature = Creature::new(g.clone(), c)?;
    let ground = creature.arena.colliders.iter().next().expect("ground").0;
    if kind == TrialKind::Rough {
        let hash = g.hash();
        let seed = u64::from_le_bytes(hash[..8].try_into().expect("eight bytes"));
        use rand::Rng;
        let mut rng = bestiary_core::rng(seed, 0x54524941, kind as u64, 0);
        for x in -6..7 {
            for z in -6..7 {
                let height = rng.gen_range(0.0..=c.combine.rough_height);
                creature.arena.colliders.insert(
                    ColliderBuilder::cuboid(0.09, height * 0.5, 0.09)
                        .translation(vector![x as f32 * 0.2, height * 0.5, z as f32 * 0.2])
                        .friction(c.physics.friction),
                );
            }
        }
    }
    if kind == TrialKind::Slope {
        let rot = Rotation::from_quaternion(Quaternion::new(
            libm::cosf(slope * 0.5),
            0.0,
            0.0,
            libm::sinf(slope * 0.5),
        ));
        creature.arena.colliders[ground].set_rotation(rot);
        for h in &creature.arena.parts {
            let p = *creature.arena.bodies[*h].position();
            creature.arena.bodies[*h].set_position(
                Isometry::from_parts(
                    Translation::from(rot * p.translation.vector),
                    rot * p.rotation,
                ),
                true,
            );
        }
    }
    if kind == TrialKind::Swim {
        creature.arena.fluid = Some(Fluid {
            level: 2.0,
            density: c.combine.water_density,
            drag: c.combine.water_drag,
        });
    }
    if kind == TrialKind::Carry {
        let mass = creature.body.mass() * c.combine.carry_fraction;
        creature.arena.bodies[creature.arena.parts[0]].set_additional_mass(mass, true);
    }
    let target = if kind == TrialKind::Strike {
        let max_x = creature
            .body
            .parts
            .iter()
            .map(|p| p.pose.translation.x + p.size[0] * 0.5)
            .fold(0.0f32, f32::max);
        Some(creature.arena.add_box(
            [max_x + 0.12, STRIKE_TARGET_SIZE[1] * 0.5, 0.0],
            STRIKE_TARGET_SIZE.map(|size| size * 0.5),
            2.0,
        ))
    } else {
        None
    };
    for handle in &creature.arena.parts {
        creature.arena.bodies[*handle]
            .recompute_mass_properties_from_colliders(&creature.arena.colliders);
    }
    // The stand interval is also the settling interval for locomotion. Energy
    // auditing remains active, but spawn/fall transients earn no movement traits.
    if locomotion(kind) {
        for _ in 0..(c.combine.stand_seconds / PHYSICS_DT).round() as usize {
            creature.step();
        }
    }
    let initial_work = creature.arena.audit.active_work;
    let initial_isometric = creature.arena.audit.isometric_cost;
    let root = creature.arena.parts[0];
    let start = center_of_mass(&creature);
    let seconds = if kind == TrialKind::Stand {
        c.combine.stand_seconds
    } else {
        c.combine.trial_seconds
    };
    let ticks = (seconds / PHYSICS_DT).round() as usize;
    let seconds = ticks as f32 * PHYSICS_DT;
    let save = matches!(kind, TrialKind::Sprint | TrialKind::Strike);
    let mut frames = if save {
        vec![creature.arena.poses()]
    } else {
        vec![]
    };
    let mut last = start;
    let mut midpoint = start;
    let mut min_root_height = root_clearance(&creature, ground);
    let mut turn = 0.0;
    let mut reach = 0.0f32;
    let mut impulse = 0.0f32;
    let mut heights = Vec::with_capacity(ticks);
    for tick in 0..ticks {
        creature.step();
        let rb = &creature.arena.bodies[root];
        let p = center_of_mass(&creature);
        last = p;
        if tick + 1 == ticks / 2 {
            midpoint = p;
        }
        min_root_height = min_root_height.min(root_clearance(&creature, ground));
        turn += rb.angvel().y * PHYSICS_DT;
        heights.push(p.y);
        for handle in creature.arena.parts.iter().take(creature.body.parts.len()) {
            for collider in creature.arena.bodies[*handle].colliders() {
                reach = reach.max(creature.arena.colliders[*collider].compute_aabb().maxs.y);
            }
        }
        if let Some(h) = target {
            impulse = impulse.max(
                creature
                    .arena
                    .interaction_impulse(h, &creature.arena.parts[..creature.body.parts.len()]),
            );
        }
        if save {
            frames.push(creature.arena.poses());
        }
    }
    let gait_frequency = if kind == TrialKind::Sprint {
        dominant_frequency(&heights, PHYSICS_DT)
    } else {
        0.0
    };
    let supported =
        !locomotion(kind) || kind == TrialKind::Swim || min_root_height >= c.combine.root_clearance;
    let distance = if supported {
        libm::sqrtf((last.x - start.x).powi(2) + (last.z - start.z).powi(2))
    } else {
        0.0
    };
    let half = (ticks / 2) as f32 * PHYSICS_DT;
    let v1 = (midpoint - start) / half;
    let v2 = (last - midpoint) / (seconds - half);
    let acceleration = libm::sqrtf((v2.x - v1.x).powi(2) + (v2.z - v1.z).powi(2)) / (seconds * 0.5);
    let mut energy = creature.arena.audit;
    energy.active_work -= initial_work;
    energy.isometric_cost -= initial_isometric;
    Ok(TrialResult {
        kind,
        skipped: false,
        speed: distance / seconds,
        acceleration: if supported { acceleration } else { 0.0 },
        distance,
        progress: if supported { last.x - start.x } else { 0.0 },
        turn_rate: if supported { turn.abs() / seconds } else { 0.0 },
        reach,
        min_root_height,
        strike_impulse: impulse,
        gait_frequency,
        energy,
        replay: if save {
            Some(Replay {
                kind,
                dt: PHYSICS_DT,
                frames,
            })
        } else {
            None
        },
    })
}
pub fn evaluate(g: &Genome, c: &Config) -> Result<Evaluation, String> {
    c.physics.validate()?;
    c.genome.validate()?;
    c.combine.validate()?;
    let body = g.expand(&c.genome)?;
    let mut trials = vec![];
    let mut traits = Traits {
        mass: body.mass(),
        volume: body
            .parts
            .iter()
            .map(|p| g.bones[p.gene].volume() * p.scale.powi(3))
            .sum(),
        parts: body.parts.len(),
        limbs: if body
            .parts
            .iter()
            .all(|p| p.children.iter().flatten().count() <= 1)
        {
            // A serial backbone has a terminal tail, not a limb.
            0
        } else {
            body.parts
                .iter()
                .skip(1)
                .filter(|p| p.children.iter().all(Option::is_none))
                .count()
        },
        muscle_area: body.muscles.iter().map(|m| m.area).sum(),
        ..Default::default()
    };
    let matched = body
        .parts
        .iter()
        .filter(|p| {
            body.parts.iter().any(|q| {
                (p.pose.translation.x - q.pose.translation.x).abs() < 0.01
                    && (p.pose.translation.z + q.pose.translation.z).abs() < 0.01
                    && p.size == q.size
            })
        })
        .count();
    traits.symmetry = matched as f32 / body.parts.len() as f32;
    let stand = trial(g, c, TrialKind::Stand, 0.0)?;
    let stands = stand.min_root_height >= c.combine.root_clearance;
    traits.standing_cost =
        (stand.energy.active_work + stand.energy.isometric_cost) as f32 / c.combine.stand_seconds;
    trials.push(stand);
    for kind in TrialKind::ALL.into_iter().skip(1) {
        if !stands && locomotion(kind) {
            trials.push(TrialResult {
                kind,
                skipped: true,
                speed: 0.0,
                acceleration: 0.0,
                distance: 0.0,
                progress: 0.0,
                turn_rate: 0.0,
                reach: 0.0,
                min_root_height: 0.0,
                strike_impulse: 0.0,
                gait_frequency: 0.0,
                energy: EnergyAudit::default(),
                replay: None,
            });
            continue;
        }
        let result = if kind == TrialKind::Slope {
            let mut best = None;
            for slope in &c.combine.slope_angles {
                let r = trial(g, c, kind, *slope)?;
                if r.progress > 0.01 && r.min_root_height >= c.combine.root_clearance {
                    traits.slope = *slope;
                }
                let mut r = r;
                if let Some(previous) = &best {
                    let previous: &TrialResult = previous;
                    r.energy.exploit_suspect |= previous.energy.exploit_suspect;
                }
                best = Some(r);
            }
            best.expect("validated nonempty slopes")
        } else {
            trial(g, c, kind, 0.0)?
        };
        match kind {
            TrialKind::Sprint => {
                traits.speed = result.speed;
                traits.acceleration = result.acceleration;
                traits.energy_per_meter = (result.energy.active_work + result.energy.isometric_cost)
                    as f32
                    / result.distance.max(1e-6);
                traits.gait_frequency = result.gait_frequency;
            }
            TrialKind::Turn => traits.turn_rate = result.turn_rate,
            TrialKind::Rough => traits.rough_speed = result.distance / c.combine.trial_seconds,
            TrialKind::Swim => traits.swim_speed = result.distance / c.combine.trial_seconds,
            TrialKind::Strike => traits.strike_impulse = result.strike_impulse,
            TrialKind::Carry => {
                if result.speed >= traits.speed * 0.5 && traits.speed > 0.001 {
                    traits.carry_mass = traits.mass * c.combine.carry_fraction;
                }
            }
            TrialKind::Reach => traits.reach = result.reach,
            _ => {}
        }
        trials.push(result);
    }
    let exploit_suspect = trials.iter().any(|t| t.energy.exploit_suspect);
    let viability = if exploit_suspect || !stands {
        0.0
    } else {
        (1.0 + traits.speed + traits.swim_speed) / (1.0 + traits.standing_cost)
    };
    Ok(Evaluation {
        genome_hash: g.hash(),
        cache_key: cache_key(g, c),
        traits,
        viability,
        exploit_suspect,
        trials,
    })
}

/// Radix-two FFT of the detrended root-height signal, excluding the DC bin.
pub fn dominant_frequency(samples: &[f32], dt: f32) -> f32 {
    if samples.len() < 4 {
        return 0.0;
    }
    let n = samples.len().next_power_of_two();
    let mean = samples.iter().sum::<f32>() / samples.len() as f32;
    let mut data = vec![(0.0f32, 0.0f32); n];
    for (i, x) in samples.iter().enumerate() {
        data[i].0 = x - mean;
    }
    let bits = n.trailing_zeros();
    for i in 0..n {
        let j = i.reverse_bits() >> (usize::BITS - bits);
        if i < j {
            data.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        for start in (0..n).step_by(len) {
            for j in 0..len / 2 {
                let angle = -std::f32::consts::TAU * j as f32 / len as f32;
                let (re, im) = data[start + j + len / 2];
                let c = libm::cosf(angle);
                let s = libm::sinf(angle);
                let v = (re * c - im * s, re * s + im * c);
                let u = data[start + j];
                data[start + j] = (u.0 + v.0, u.1 + v.1);
                data[start + j + len / 2] = (u.0 - v.0, u.1 - v.1);
            }
        }
        len *= 2;
    }
    let mut best = 0usize;
    let mut power = 1e-10;
    for (i, (re, im)) in data.iter().enumerate().take(n / 2).skip(1) {
        let p = re * re + im * im;
        if p > power {
            best = i;
            power = p;
        }
    }
    best as f32 / (n as f32 * dt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn inert_root(shape: bestiary_core::genome::Shape, config: &Config) -> Genome {
        let mut g = Genome::starter(&config.genome);
        g.bones[0].shape = shape;
        g.bones[0].edges.clear();
        g.bones[0].muscles.clear();
        g.bones.truncate(1);
        g.repair(&config.genome);
        g
    }
    #[test]
    fn stand_gate_keeps_root_surface_off_ground_for_two_seconds() {
        let config = Config::bundled();
        for shape in [
            bestiary_core::genome::Shape::Box,
            bestiary_core::genome::Shape::Capsule,
        ] {
            let result = evaluate(&inert_root(shape, &config), &config).unwrap();
            assert!(result.trials[0].min_root_height < config.combine.root_clearance);
            assert_eq!(result.viability, 0.0);
            assert_eq!(result.traits.speed, 0.0);
            assert!(result
                .trials
                .iter()
                .filter(|t| locomotion(t.kind))
                .all(|t| t.skipped));
        }
        let healthy = evaluate(&Genome::starter(&config.genome), &config).unwrap();
        assert!(healthy.trials[0].min_root_height >= config.combine.root_clearance);
        assert!(healthy.viability > 0.0);
    }
    #[test]
    fn root_clearance_accounts_for_shape_rotation_and_ground_plane() {
        let config = Config::bundled();
        let g = inert_root(bestiary_core::genome::Shape::Box, &config);
        let mut creature = Creature::new(g, &config).unwrap();
        let ground = creature.arena.colliders.iter().next().unwrap().0;
        let root_collider = creature.arena.bodies[creature.arena.parts[0]].colliders()[0];
        creature.arena.colliders[root_collider].set_position(Isometry::new(
            vector![0.0, 1.0, 0.0],
            vector![0.0, 0.0, std::f32::consts::FRAC_PI_2],
        ));
        assert!((root_clearance(&creature, ground) - 0.35).abs() < 1e-6);
        creature.arena.colliders[ground].set_translation(vector![0.0, 0.1, 0.0]);
        assert!((root_clearance(&creature, ground) - 0.15).abs() < 1e-6);
    }
    #[test]
    fn root_on_raised_terrain_fails_clearance_despite_height_above_base_plane() {
        let config = Config::bundled();
        let g = inert_root(bestiary_core::genome::Shape::Box, &config);
        let mut creature = Creature::new(g, &config).unwrap();
        let ground = creature.arena.colliders.iter().next().unwrap().0;
        let root = creature.arena.parts[0];
        creature.arena.bodies[root].set_translation(vector![0.0, 1.0, 0.0], true);
        creature
            .arena
            .colliders
            .insert(ColliderBuilder::cuboid(2.0, 0.1, 2.0).translation(vector![0.0, 0.3, 0.0]));
        for _ in 0..480 {
            creature.step();
        }
        let collider = creature.arena.bodies[root].colliders()[0];
        assert!(creature.arena.colliders[collider].compute_aabb().mins.y > 0.3);
        assert!(creature.arena.touches_terrain(root));
        assert_eq!(root_clearance(&creature, ground), 0.0);
    }
    #[test]
    fn stand_duration_uses_stand_seconds_independently_of_trial_duration() {
        let mut config = Config::bundled();
        let g = Genome::starter(&config.genome);
        let original = trial(&g, &config, TrialKind::Stand, 0.0).unwrap();
        config.combine.trial_seconds = config.combine.stand_seconds * 2.0;
        assert_eq!(original, trial(&g, &config, TrialKind::Stand, 0.0).unwrap());
    }
    #[test]
    fn sprint_excludes_initial_settling_and_passive_collapse() {
        let config = Config::bundled();
        // Keep the original rigid-leg regression body stable as authored zoo
        // anatomy evolves; walking and slip have their own founder tests.
        let active = Genome::rigid_limb_fixture(&config.genome);
        let mut passive = active.clone();
        for bone in &mut passive.bones {
            for neuron in &mut bone.motor.neurons {
                neuron.bias = -1.0;
                neuron.activation = bestiary_core::brain::Activation::Threshold;
                neuron.inputs.clear();
            }
        }
        let passive_trial = trial(&passive, &config, TrialKind::Sprint, 0.0).unwrap();
        assert!(
            passive_trial.speed < 0.001,
            "passive speed={} distance={}",
            passive_trial.speed,
            passive_trial.distance
        );
        let active_trial = trial(&active, &config, TrialKind::Sprint, 0.0).unwrap();
        assert!(active_trial.speed > 0.01 && active_trial.speed > passive_trial.speed * 10.0);
        // A one-legged body can move a long way by tipping and falling. It may
        // not turn that passive potential-energy release into a locomotion trait.
        passive.bones[0].edges.truncate(1);
        passive.repair(&config.genome);
        let fallen = trial(&passive, &config, TrialKind::Sprint, 0.0).unwrap();
        assert!(fallen.min_root_height < config.combine.root_clearance);
        assert_eq!(fallen.speed, 0.0);
        assert_eq!(fallen.distance, 0.0);
        assert_eq!(fallen.acceleration, 0.0);
        assert_eq!(fallen.energy.active_work, 0.0);
    }
    #[test]
    fn cache_hit_equals_recompute_and_parallel_equals_serial() {
        let config = Config::bundled();
        let starter = Genome::starter(&config.genome);
        let other = starter
            .mutate(
                bestiary_core::genome::Operator::LifeHistory,
                99,
                1,
                &config.genome,
            )
            .0;
        let mut cache = Combine::default();
        let fresh = evaluate(&starter, &config).unwrap();
        println!(
            "starter traits={:?}, viability={}, exploit={}",
            fresh.traits, fresh.viability, fresh.exploit_suspect
        );
        for t in &fresh.trials {
            println!(
                "{:?} energy={:?} height={}",
                t.kind, t.energy, t.min_root_height
            );
        }
        assert_eq!(fresh, cache.evaluate(&starter, &config).unwrap());
        assert_eq!(fresh, cache.evaluate(&starter, &config).unwrap());
        let genomes = [starter, other];
        assert_eq!(
            cache.evaluate_many(&genomes, &config, false).unwrap(),
            cache.evaluate_many(&genomes, &config, true).unwrap()
        );
        assert_eq!(fresh.trials.len(), 9);
        assert!(
            !fresh.exploit_suspect,
            "ordinary preset must not trigger energy detector"
        );
        for t in fresh.trials {
            if let Some(replay) = t.replay {
                assert_eq!(Replay::decode(&replay.encode().unwrap()).unwrap(), replay);
            }
        }
    }
    #[test]
    fn deliberate_solver_exploit_is_flagged() {
        let config = Config::bundled();
        let mut g = Genome::starter(&config.genome);
        // Identical siblings overlap, so collision stabilization competes with joint constraints.
        let mut e = g.bones[0].edges[0].clone();
        e.attachment = [0.0, -1.0, 0.0];
        e.kind = bestiary_core::genome::JointKind::Fixed;
        g.bones[0].edges = vec![e; 8];
        g.repair(&config.genome);
        let result = evaluate(&g, &config).unwrap();
        println!(
            "exploit={} audits={:?}",
            result.exploit_suspect,
            result.trials.iter().map(|t| &t.energy).collect::<Vec<_>>()
        );
        assert!(result.exploit_suspect);
        assert_eq!(result.viability, 0.0);
    }
    #[test]
    fn fft_recovers_dominant_gait_frequency() {
        let signal: Vec<_> = (0..512)
            .map(|i| libm::sinf(std::f32::consts::TAU * 4.0 * i as f32 / 256.0) + 7.0)
            .collect();
        assert_eq!(dominant_frequency(&signal, 1.0 / 256.0), 4.0);
        assert_eq!(dominant_frequency(&[1.0; 512], 1.0 / 256.0), 0.0);
    }
    #[test]
    fn combine_configuration_and_cache_invalidation() {
        let mut config = Config::bundled();
        let g = Genome::starter(&config.genome);
        let old = cache_key(&g, &config);
        config.physics.force_per_area *= 2.0;
        assert_ne!(old, cache_key(&g, &config));
        config.combine.trial_seconds = 0.0;
        assert!(config.combine.validate().is_err());
        config = Config::bundled();
        config.combine.root_clearance = 0.0;
        assert!(config.combine.validate().is_err());
        config = Config::bundled();
        config.combine.slope_angles.clear();
        assert!(config.combine.validate().is_err());
    }
    #[test]
    fn preset_zoo_traits_are_stable_across_runs() {
        let config = Config::bundled();
        for (name, g) in bestiary_core::genome::zoo(&config.genome) {
            assert_eq!(
                Genome::from_share(&g.to_share(), &config.genome).unwrap(),
                g
            );
            let first = evaluate(&g, &config).unwrap();
            let second = evaluate(&g, &config).unwrap();
            assert_eq!(first, second, "{name}");
            assert!(!first.exploit_suspect, "{name} triggers the energy audit");
            assert!(first.viability > 0.0, "{name} must pass the stand gate");
            assert!(
                first.trials[0].min_root_height >= config.combine.root_clearance,
                "{name}"
            );
            if name == "serpent" {
                assert_eq!(first.traits.limbs, 0);
            }
        }
    }
}
