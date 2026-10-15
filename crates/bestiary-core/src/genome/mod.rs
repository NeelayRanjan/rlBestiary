//! Heritable graph encoding, deterministic expansion and genetic operators.
mod expand;
mod genetics;
use crate::brain::Brain;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
pub use expand::*;
pub use genetics::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub max_parts: usize,
    pub max_depth: usize,
    pub max_mass: f32,
    pub min_size: f32,
    pub max_span: usize,
    pub mutation_floor: f32,
    pub mutation_ceiling: f32,
    pub attachment_jitter: f32,
}
impl Limits {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=32).contains(&self.max_parts)
            || !(1..=12).contains(&self.max_depth)
            || !self.max_mass.is_finite()
            || self.max_mass <= 0.0
            || !self.min_size.is_finite()
            || self.min_size <= 0.0
            || !(1..=2).contains(&self.max_span)
            || !self.mutation_floor.is_finite()
            || !self.mutation_ceiling.is_finite()
            || self.mutation_floor < 0.0
            || self.mutation_ceiling > 1.0
            || self.mutation_floor > self.mutation_ceiling
            || !self.attachment_jitter.is_finite()
            || !(0.0..=0.5).contains(&self.attachment_jitter)
        {
            return Err("invalid genome limits".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum Shape {
    Box,
    Capsule,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum JointKind {
    Fixed,
    Hinge,
    Universal,
    Ball,
}
impl JointKind {
    pub fn axes(self) -> &'static [usize] {
        match self {
            Self::Fixed => &[],
            Self::Hinge => &[2],
            Self::Universal => &[0, 2],
            Self::Ball => &[0, 1, 2],
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Sensor {
    JointAngle(usize),
    JointVelocity(usize),
    MuscleLength(usize),
    MuscleVelocity(usize),
    Contact,
    ContactForce,
    Gravity(usize),
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Edge {
    pub child: usize,
    /// Normalized parent surface coordinates in [-1,1].
    pub attachment: [f32; 3],
    pub child_attachment: [f32; 3],
    /// Euler angles in radians, evaluated through libm.
    pub orientation: [f32; 3],
    pub scale: f32,
    pub kind: JointKind,
    pub limits: [[f32; 2]; 3],
    pub reflect: bool,
    pub terminal_only: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Route {
    Parent,
    Child(usize),
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MuscleProperties {
    pub contraction_scale: f32,
    pub stiffness_scale: f32,
    pub damping_scale: f32,
}
impl Default for MuscleProperties {
    fn default() -> Self {
        Self {
            contraction_scale: 1.0,
            stiffness_scale: 1.0,
            damping_scale: 1.0,
        }
    }
}
impl MuscleProperties {
    pub fn valid(&self) -> bool {
        [
            self.contraction_scale,
            self.stiffness_scale,
            self.damping_scale,
        ]
        .iter()
        .all(|x| x.is_finite() && (0.1..=2.0).contains(x))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MuscleGene {
    pub properties: MuscleProperties,
    pub route: Vec<Route>,
    pub origin: [f32; 3],
    pub insertion: [f32; 3],
    pub area: f32,
    pub output: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Bone {
    pub shape: Shape,
    /// Full bounding-box dimensions. Capsule radius uses min(x,z)/2.
    pub size: [f32; 3],
    pub density: f32,
    pub color: [f32; 3],
    pub recursion_limit: u8,
    pub sensors: Vec<Sensor>,
    pub edges: Vec<Edge>,
    pub muscles: Vec<MuscleGene>,
    pub motor: Brain,
}
impl Bone {
    pub fn volume(&self) -> f32 {
        match self.shape {
            Shape::Box => self.size.iter().product(),
            Shape::Capsule => {
                let r = self.size[0].min(self.size[2]) * 0.5;
                std::f32::consts::PI * r * r * ((self.size[1] - 2.0 * r).max(0.0) + 4.0 * r / 3.0)
            }
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LifeHistory {
    pub plant_efficiency: f32,
    pub offspring_fraction: f32,
    pub clutch_size: u8,
    pub maturity: u64,
    pub senescence: u64,
    pub immune_investment: f32,
    pub resistance: [f32; 4],
    pub pheromone_channel: u8,
    pub pheromone_strength: f32,
    pub mate_weights: [f32; 4],
    pub species_marker: [f32; 4],
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Genome {
    pub root: usize,
    pub bones: Vec<Bone>,
    pub ethology: Brain,
    pub life: LifeHistory,
    pub mutation_rates: Vec<f32>,
}
fn surface(p: &[f32; 3]) -> bool {
    p.iter().all(|x| x.is_finite() && x.abs() <= 1.0) && p.iter().any(|x| x.abs() == 1.0)
}
impl Genome {
    pub fn validate(&self, limits: &Limits) -> Result<(), String> {
        limits.validate()?;
        if self.bones.is_empty()
            || self.bones.len() > 64
            || self.root >= self.bones.len()
            || !self.ethology.valid()
            || self.mutation_rates.len() != Operator::ALL.len()
            || self.mutation_rates.iter().any(|r| {
                !r.is_finite() || *r < limits.mutation_floor || *r > limits.mutation_ceiling
            })
        {
            return Err("invalid genome root, rates or ethology brain".into());
        }
        for b in &self.bones {
            if b.size
                .iter()
                .any(|v| !v.is_finite() || *v < limits.min_size || *v > 10.0)
                || !b.density.is_finite()
                || !(0.01..=100.0).contains(&b.density)
                || b.volume() * b.density > limits.max_mass
                || b.recursion_limit == 0
                || b.recursion_limit > 12
                || b.color
                    .iter()
                    .any(|x| !x.is_finite() || !(0.0..=1.0).contains(x))
                || b.edges.len() > 8
                || b.muscles.len() > 128
                || b.sensors.len() > 32
                || !b.motor.valid()
            {
                return Err("invalid bone".into());
            }
            for e in &b.edges {
                if e.child >= self.bones.len()
                    || !surface(&e.attachment)
                    || !surface(&e.child_attachment)
                    || !e.scale.is_finite()
                    || !(0.25..=2.0).contains(&e.scale)
                    || e.orientation.iter().any(|x| !x.is_finite())
                    || e.limits.iter().any(|v| {
                        !v[0].is_finite()
                            || !v[1].is_finite()
                            || v[0] > v[1]
                            || v[0] < -std::f32::consts::PI
                            || v[1] > std::f32::consts::PI
                    })
                {
                    return Err("invalid joint".into());
                }
            }
            for m in &b.muscles {
                if !m.properties.valid()
                    || m.route.is_empty()
                    || m.route.len() > limits.max_span
                    || !surface(&m.origin)
                    || !surface(&m.insertion)
                    || !m.area.is_finite()
                    || !(0.001..=10.0).contains(&m.area)
                    || m.output >= b.motor.neurons.len()
                {
                    return Err("invalid muscle".into());
                }
            }
        }
        let l = &self.life;
        if [
            l.plant_efficiency,
            l.offspring_fraction,
            l.immune_investment,
            l.pheromone_strength,
        ]
        .iter()
        .chain(&l.resistance)
        .chain(&l.species_marker)
        .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            || l.mate_weights
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1.0)
            || l.clutch_size == 0
            || l.clutch_size > 16
            || l.maturity == 0
            || l.senescence <= l.maturity
            || l.pheromone_channel >= 4
        {
            return Err("invalid life history".into());
        }
        Ok(())
    }
    pub fn hash(&self) -> [u8; 32] {
        crate::digest(self).expect("genome serializes")
    }
    pub fn to_share(&self) -> String {
        URL_SAFE_NO_PAD.encode(postcard::to_allocvec(self).expect("genome serializes"))
    }
    pub fn from_share(s: &str, limits: &Limits) -> Result<Self, String> {
        if s.len() > 1_000_000 {
            return Err("share string too large".into());
        }
        let bytes = URL_SAFE_NO_PAD.decode(s).map_err(|e| e.to_string())?;
        let (g, tail): (Self, &[u8]) =
            postcard::take_from_bytes(&bytes).map_err(|e| e.to_string())?;
        if !tail.is_empty() {
            return Err("trailing genome data".into());
        }
        g.validate(limits)?;
        Ok(g)
    }
    pub fn starter(limits: &Limits) -> Self {
        let root = Bone {
            shape: Shape::Box,
            size: [1.3, 0.24, 0.5],
            density: 4.0,
            color: [0.3, 0.6, 0.4],
            recursion_limit: 1,
            sensors: vec![Sensor::Contact, Sensor::Gravity(1)],
            edges: vec![],
            muscles: vec![],
            motor: Brain::oscillator(0.0),
        };
        let mut g = Self {
            root: 0,
            bones: vec![root],
            ethology: Brain::oscillator(0.0),
            life: LifeHistory {
                plant_efficiency: 0.8,
                offspring_fraction: 0.15,
                clutch_size: 1,
                maturity: 200,
                senescence: 2000,
                immune_investment: 0.1,
                resistance: [0.5; 4],
                pheromone_channel: 0,
                pheromone_strength: 0.1,
                mate_weights: [0.25; 4],
                species_marker: [0.5; 4],
            },
            mutation_rates: vec![
                0.1f32.clamp(limits.mutation_floor, limits.mutation_ceiling);
                Operator::ALL.len()
            ],
        };
        for row in 0..3 {
            for side in 0..2 {
                let id = g.bones.len();
                g.bones.push(Bone {
                    shape: Shape::Box,
                    size: [0.09, 0.6, 0.09],
                    density: 4.0,
                    color: [0.5, 0.7, 0.3],
                    recursion_limit: 1,
                    sensors: vec![
                        Sensor::JointAngle(2),
                        Sensor::JointVelocity(2),
                        Sensor::Contact,
                    ],
                    edges: vec![],
                    muscles: vec![],
                    motor: Brain::oscillator(((row + side) % 2) as f32 * std::f32::consts::PI),
                });
                for (output, sign) in [-1.0, 1.0].into_iter().enumerate() {
                    g.bones[id].muscles.push(MuscleGene {
                        properties: MuscleProperties::default(),
                        route: vec![Route::Parent],
                        origin: [sign, 0.0, 0.0],
                        insertion: [
                            ((row as f32 - 1.0) * 0.65 + sign * 0.18).clamp(-1.0, 1.0),
                            -1.0,
                            if side == 0 { -1.0 } else { 1.0 },
                        ],
                        area: 0.1,
                        output,
                    });
                }
                g.bones[0].edges.push(Edge {
                    child: id,
                    attachment: [
                        (row as f32 - 1.0) * 0.65,
                        -0.5,
                        if side == 0 { -1.0 } else { 1.0 },
                    ],
                    child_attachment: [0.0, 1.0, 0.0],
                    orientation: [0.0; 3],
                    scale: 1.0,
                    kind: JointKind::Hinge,
                    limits: [[-0.7, 0.7]; 3],
                    reflect: false,
                    terminal_only: false,
                });
            }
        }
        g.repair(limits);
        g
    }
}

#[cfg(test)]
mod tests;
