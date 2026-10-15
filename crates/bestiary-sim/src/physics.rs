use crate::{config::PhysicsConfig, muscle, PHYSICS_DT};
use rapier3d::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct Muscle {
    pub properties: bestiary_core::genome::MuscleProperties,
    pub origin_body: RigidBodyHandle,
    pub insertion_body: RigidBodyHandle,
    pub origin: Point<Real>,
    pub insertion: Point<Real>,
    pub rest: f32,
    pub area: f32,
    pub phase: f32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnergyAudit {
    pub active_work: f64,
    pub isometric_cost: f64,
    pub unexplained_gain: f64,
    pub exploit_suspect: bool,
}
impl EnergyAudit {
    pub fn observe(&mut self, before: f64, after: f64, work: f64, config: &PhysicsConfig) {
        self.active_work += work.max(0.0);
        let residual = (after - before - work.max(0.0)).max(0.0);
        let tolerance = f64::from(config.audit_absolute_tolerance)
            + f64::from(config.audit_relative_tolerance) * before.abs().max(after.abs());
        self.unexplained_gain += residual;
        self.exploit_suspect |=
            !after.is_finite() || !before.is_finite() || !work.is_finite() || residual > tolerance;
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Pose {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Trial {
    pub frames: Vec<Vec<Pose>>,
    pub energy: EnergyAudit,
    pub displacement: f32,
    pub min_root_height: f32,
}

pub struct Fluid {
    pub level: f32,
    pub density: f32,
    pub drag: f32,
}
pub struct Arena {
    pub fluid: Option<Fluid>,
    pub config: PhysicsConfig,
    pub bodies: RigidBodySet,
    pub colliders: ColliderSet,
    pub joints: ImpulseJointSet,
    pub parts: Vec<RigidBodyHandle>,
    pub muscles: Vec<Muscle>,
    pipeline: PhysicsPipeline,
    islands: IslandManager,
    broad: BroadPhaseMultiSap,
    narrow: NarrowPhase,
    multibody: MultibodyJointSet,
    ccd: CCDSolver,
    pub tick: u64,
    pub audit: EnergyAudit,
}
impl Arena {
    pub fn new(config: PhysicsConfig) -> Result<Self, String> {
        config.validate()?;
        let mut colliders = ColliderSet::new();
        colliders.insert(
            ColliderBuilder::cuboid(100.0, 0.1, 100.0)
                .translation(vector![0.0, -0.1, 0.0])
                .friction(config.friction)
                .restitution(0.0),
        );
        Ok(Self {
            fluid: None,
            config,
            bodies: RigidBodySet::new(),
            colliders,
            joints: ImpulseJointSet::new(),
            parts: vec![],
            muscles: vec![],
            pipeline: PhysicsPipeline::new(),
            islands: IslandManager::new(),
            broad: BroadPhaseMultiSap::new(),
            narrow: NarrowPhase::new(),
            multibody: MultibodyJointSet::new(),
            ccd: CCDSolver::new(),
            tick: 0,
            audit: EnergyAudit::default(),
        })
    }
    pub fn add_box(&mut self, center: [f32; 3], half: [f32; 3], density: f32) -> RigidBodyHandle {
        let h = self.bodies.insert(
            RigidBodyBuilder::dynamic()
                .translation(center.into())
                .can_sleep(false)
                .ccd_enabled(true),
        );
        self.colliders.insert_with_parent(
            ColliderBuilder::cuboid(half[0], half[1], half[2])
                .density(density)
                .friction(self.config.friction)
                .restitution(0.0),
            h,
            &mut self.bodies,
        );
        self.parts.push(h);
        h
    }
    pub fn add_muscle(
        &mut self,
        a: RigidBodyHandle,
        b: RigidBodyHandle,
        origin: Point<Real>,
        insertion: Point<Real>,
        area: f32,
        phase: f32,
    ) {
        let rest =
            ((self.bodies[a].position() * origin) - (self.bodies[b].position() * insertion)).norm();
        assert!(rest > 0.0001 && area > 0.0);
        self.muscles.push(Muscle {
            properties: Default::default(),
            origin_body: a,
            insertion_body: b,
            origin,
            insertion,
            rest,
            area,
            phase,
        });
    }
    pub fn endpoints(&self, m: &Muscle) -> (Point<Real>, Point<Real>) {
        (
            self.bodies[m.origin_body].position() * m.origin,
            self.bodies[m.insertion_body].position() * m.insertion,
        )
    }
    pub fn mechanical_energy(&self) -> f64 {
        let mut energy = 0.0f64;
        for h in &self.parts {
            let b = &self.bodies[*h];
            energy += f64::from(b.kinetic_energy())
                + f64::from(b.mass() * self.config.gravity * b.center_of_mass().y);
        }
        for m in &self.muscles {
            let (a, b) = self.endpoints(m);
            energy += f64::from(
                muscle::force_with_properties(
                    &self.config,
                    &m.properties,
                    m.rest,
                    (b - a).norm(),
                    0.0,
                    m.area,
                    0.0,
                )
                .spring_energy,
            );
        }
        energy
    }
    pub fn interaction_impulse(
        &self,
        target: RigidBodyHandle,
        creature: &[RigidBodyHandle],
    ) -> f32 {
        self.bodies[target]
            .colliders()
            .iter()
            .flat_map(|h| self.narrow.contact_pairs_with(*h))
            .filter(|pair| {
                [pair.collider1, pair.collider2].iter().any(|h| {
                    self.colliders[*h]
                        .parent()
                        .is_some_and(|p| creature.contains(&p))
                })
            })
            .map(|pair| pair.total_impulse().norm())
            .sum()
    }
    pub fn contact_force(&self, handle: RigidBodyHandle) -> f32 {
        self.bodies[handle]
            .colliders()
            .iter()
            .flat_map(|h| self.narrow.contact_pairs_with(*h))
            .map(|pair| pair.total_impulse().norm() / PHYSICS_DT)
            .sum()
    }
    pub fn poses(&self) -> Vec<Pose> {
        self.parts
            .iter()
            .map(|h| {
                let b = &self.bodies[*h];
                Pose {
                    translation: (*b.translation()).into(),
                    rotation: b.rotation().coords.into(),
                }
            })
            .collect()
    }
    /// Exactly one fixed tick; activations are held for every configured substep.
    pub fn step(&mut self, activations: &[f32]) {
        assert_eq!(activations.len(), self.muscles.len());
        if self.tick == 0 {
            for h in &self.parts {
                self.bodies[*h].recompute_mass_properties_from_colliders(&self.colliders);
            }
        }
        let dt = PHYSICS_DT / self.config.substeps as f32;
        for _ in 0..self.config.substeps {
            let before = self.mechanical_energy();
            for h in &self.parts {
                self.bodies[*h].reset_forces(false);
                self.bodies[*h].reset_torques(false);
            }
            let mut environment = Vec::new();
            if let Some(fluid) = &self.fluid {
                for h in &self.parts {
                    let b = &self.bodies[*h];
                    let mut volume = 0.0;
                    for c in b.colliders() {
                        let collider = &self.colliders[*c];
                        let bounds = collider.compute_aabb();
                        let fraction = ((fluid.level - bounds.mins.y)
                            / (bounds.maxs.y - bounds.mins.y).max(1e-6))
                        .clamp(0.0, 1.0);
                        volume += collider.volume() * fraction;
                    }
                    let force = Vector::y() * (volume * fluid.density * self.config.gravity)
                        - b.linvel() * (fluid.drag * volume);
                    environment.push((*h, *b.center_of_mass(), force));
                }
                for (h, _, force) in &environment {
                    self.bodies[*h].add_force(*force, true);
                }
            }
            let mut work_terms = Vec::with_capacity(self.muscles.len());
            for (m, activation) in self.muscles.iter().zip(activations) {
                let (a, b) = self.endpoints(m);
                let delta = b - a;
                let length = delta.norm().max(0.00001);
                let direction = delta / length;
                let velocity = (self.bodies[m.insertion_body].velocity_at_point(&b)
                    - self.bodies[m.origin_body].velocity_at_point(&a))
                .dot(&direction);
                let f = muscle::force_with_properties(
                    &self.config,
                    &m.properties,
                    m.rest,
                    length,
                    velocity,
                    m.area,
                    *activation,
                );
                self.bodies[m.origin_body].add_force_at_point(
                    direction * (f.active + f.passive),
                    a,
                    true,
                );
                self.bodies[m.insertion_body].add_force_at_point(
                    -direction * (f.active + f.passive),
                    b,
                    true,
                );
                self.audit.isometric_cost += f64::from(f.isometric_power * dt);
                work_terms.push((a, b, direction * f.active));
            }
            let parameters = IntegrationParameters {
                dt,
                ..Default::default()
            };
            self.pipeline.step(
                &vector![0.0, -self.config.gravity, 0.0],
                &parameters,
                &mut self.islands,
                &mut self.broad,
                &mut self.narrow,
                &mut self.bodies,
                &mut self.colliders,
                &mut self.joints,
                &mut self.multibody,
                &mut self.ccd,
                None,
                &(),
                &(),
            );
            let work: f64 = self
                .muscles
                .iter()
                .zip(work_terms)
                .map(|(m, (a, b, force))| {
                    let (next_a, next_b) = self.endpoints(m);
                    f64::from(force.dot(&((next_a - a) - (next_b - b))).max(0.0))
                })
                .sum();
            let environmental_work: f64 = environment
                .iter()
                .map(|(h, p, f)| f64::from(f.dot(&(self.bodies[*h].center_of_mass() - p))))
                .sum();
            self.audit.observe(
                before,
                self.mechanical_energy() - environmental_work,
                work,
                &self.config,
            );
        }
        self.tick += 1;
    }
}

/// Twelve-part mechanical fixture: trunk, ten legs and a fixed head.
/// All gait actuation is via antagonist linear muscles, never joint motors.
pub fn walker(config: PhysicsConfig) -> Arena {
    let mut a = Arena::new(config).expect("valid fixture config");
    let root = a.add_box([0.0, 0.72, 0.0], [0.65, 0.12, 0.25], 4.0);
    for row in 0..5 {
        for side in 0..2 {
            let x = -0.48 + row as f32 * 0.24;
            let z = if side == 0 { -0.34 } else { 0.34 };
            let leg = a.add_box([x, 0.36, z], [0.045, 0.3, 0.045], 4.0);
            a.joints.insert(
                root,
                leg,
                RevoluteJointBuilder::new(Vector::z_axis())
                    .local_anchor1(point![x, -0.06, z])
                    .local_anchor2(point![0.0, 0.3, 0.0])
                    .limits([-0.7, 0.7])
                    .contacts_enabled(false),
                true,
            );
            let phase = ((row + side) % 2) as f32 * std::f32::consts::PI;
            for sign in [-1.0f32, 1.0] {
                a.add_muscle(
                    root,
                    leg,
                    point![x + sign * 0.12, -0.12, z.signum() * 0.25],
                    point![0.0, 0.0, 0.0],
                    a.config.fixture_muscle_area,
                    phase
                        + if sign < 0.0 {
                            std::f32::consts::PI
                        } else {
                            0.0
                        },
                );
            }
        }
    }
    let head = a.add_box([0.8, 0.72, 0.0], [0.12, 0.1, 0.15], 4.0);
    a.joints.insert(
        root,
        head,
        FixedJointBuilder::new()
            .local_anchor1(point![0.65, 0.0, 0.0])
            .local_anchor2(point![-0.15, 0.0, 0.0])
            .contacts_enabled(false),
        true,
    );
    a
}

pub fn run_fixture(config: PhysicsConfig, ticks: u32, driven: bool) -> Trial {
    let mut a = walker(config);
    let start = *a.bodies[a.parts[0]].translation();
    let mut frames = vec![a.poses()];
    let mut min_root_height = start.y;
    for _ in 0..ticks {
        let t = a.tick as f32 * PHYSICS_DT;
        let activation: Vec<_> = a
            .muscles
            .iter()
            .map(|m| {
                if driven {
                    (0.5 + 0.5
                        * libm::sinf(
                            t * std::f32::consts::TAU * a.config.fixture_frequency + m.phase,
                        ))
                    .max(0.0)
                } else {
                    0.0
                }
            })
            .collect();
        a.step(&activation);
        min_root_height = min_root_height.min(a.bodies[a.parts[0]].translation().y);
        frames.push(a.poses());
    }
    let end = *a.bodies[a.parts[0]].translation();
    Trial {
        frames,
        energy: a.audit,
        displacement: libm::sqrtf((end.x - start.x).powi(2) + (end.z - start.z).powi(2)),
        min_root_height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    #[test]
    fn deterministic_identical_physics_trials() {
        let config = Config::bundled().physics;
        let a = run_fixture(config.clone(), 240, true);
        let b = run_fixture(config, 240, true);
        assert_eq!(a, b);
        assert_eq!(
            bestiary_core::digest(&a).unwrap(),
            bestiary_core::digest(&b).unwrap()
        );
    }
    #[test]
    fn hand_built_body_walks_under_muscle_actuation() {
        let config = Config::bundled().physics;
        let active = run_fixture(config.clone(), 1440, true);
        let passive = run_fixture(config, 1440, false);
        println!(
            "active distance={} passive={} height={} audit={:?}",
            active.displacement, passive.displacement, active.min_root_height, active.energy
        );
        assert!(active.displacement > 0.05 && active.displacement > passive.displacement * 2.0);
        assert!(active.min_root_height > 0.25);
        assert!(active.energy.active_work > 0.0 && active.energy.isometric_cost > 0.0);
        assert!(!active.energy.exploit_suspect);
    }
    #[test]
    fn free_energy_audit_flags_unbudgeted_energy_and_accepts_work() {
        let c = Config::bundled().physics;
        let mut audit = EnergyAudit::default();
        audit.observe(10.0, 11.0, 1.0, &c);
        assert!(!audit.exploit_suspect);
        audit.observe(11.0, 111.0, 0.0, &c);
        assert!(audit.exploit_suspect);
        let mut invalid = EnergyAudit::default();
        invalid.observe(1.0, f64::NAN, 0.0, &c);
        assert!(invalid.exploit_suspect);
    }
}
