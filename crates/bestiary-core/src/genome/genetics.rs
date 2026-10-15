use super::*;
use crate::brain::{Activation, Connection, Input, Neuron};
use rand::Rng;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum Operator {
    Parameter,
    AddBone,
    RemoveBone,
    AddJoint,
    RemoveJoint,
    JointType,
    Recursion,
    Reflection,
    DuplicateSubtree,
    PruneSubtree,
    AddMuscle,
    RemoveMuscle,
    MoveAttachment,
    MuscleArea,
    MuscleSpan,
    AddSensor,
    RemoveSensor,
    AddNeuron,
    RemoveNeuron,
    Weights,
    Activation,
    Plasticity,
    LifeHistory,
}
impl Operator {
    pub const ALL: [Self; 23] = [
        Self::Parameter,
        Self::AddBone,
        Self::RemoveBone,
        Self::AddJoint,
        Self::RemoveJoint,
        Self::JointType,
        Self::Recursion,
        Self::Reflection,
        Self::DuplicateSubtree,
        Self::PruneSubtree,
        Self::AddMuscle,
        Self::RemoveMuscle,
        Self::MoveAttachment,
        Self::MuscleArea,
        Self::MuscleSpan,
        Self::AddSensor,
        Self::RemoveSensor,
        Self::AddNeuron,
        Self::RemoveNeuron,
        Self::Weights,
        Self::Activation,
        Self::Plasticity,
        Self::LifeHistory,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Parameter => "parameter_jitter",
            Self::AddBone => "add_bone",
            Self::RemoveBone => "remove_bone",
            Self::AddJoint => "add_joint",
            Self::RemoveJoint => "remove_joint",
            Self::JointType => "joint_type",
            Self::Recursion => "recursion_limit",
            Self::Reflection => "reflection",
            Self::DuplicateSubtree => "duplicate_subtree",
            Self::PruneSubtree => "prune_subtree",
            Self::AddMuscle => "add_muscle",
            Self::RemoveMuscle => "remove_muscle",
            Self::MoveAttachment => "move_attachment",
            Self::MuscleArea => "muscle_area",
            Self::MuscleSpan => "muscle_span",
            Self::AddSensor => "add_sensor",
            Self::RemoveSensor => "remove_sensor",
            Self::AddNeuron => "add_neuron",
            Self::RemoveNeuron => "remove_neuron",
            Self::Weights => "weights",
            Self::Activation => "activation",
            Self::Plasticity => "plasticity",
            Self::LifeHistory => "life_history",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MutationRecord {
    pub operator: String,
    pub parent: [u8; 32],
    pub child: [u8; 32],
    pub traits: Option<Vec<f32>>,
    pub viability: Option<f32>,
    pub born: bool,
    pub lifespan: Option<u64>,
    pub lineage_survival: Option<u64>,
}
impl Genome {
    /// Restore local references and inherited joint-spanning muscle genes.
    pub fn repair(&mut self, limits: &Limits) {
        let mut reachable = BTreeSet::new();
        let mut pending = vec![self.root];
        while let Some(i) = pending.pop() {
            if i < self.bones.len() && reachable.insert(i) {
                pending.extend(self.bones[i].edges.iter().map(|e| e.child));
            }
        }
        let remap: BTreeMap<_, _> = reachable
            .iter()
            .enumerate()
            .map(|(new, old)| (*old, new))
            .collect();
        if reachable.len() != self.bones.len() {
            self.bones = reachable
                .iter()
                .map(|i| {
                    let mut b = self.bones[*i].clone();
                    b.edges.retain(|e| remap.contains_key(&e.child));
                    for e in &mut b.edges {
                        e.child = remap[&e.child];
                    }
                    b
                })
                .collect();
            self.root = remap[&self.root];
        }
        let len = self.bones.len();
        let parent_muscles: Vec<_> = self
            .bones
            .iter()
            .map(|b| {
                b.muscles
                    .iter()
                    .filter(|m| m.route == [Route::Parent])
                    .count()
            })
            .collect();
        for b in &mut self.bones {
            b.edges.retain(|e| e.child < len);
            for e in &mut b.edges {
                e.scale = e.scale.clamp(0.25, 2.0);
            }
            b.muscles
                .retain(|m| !m.route.is_empty() && m.route.len() <= limits.max_span);
            for m in &mut b.muscles {
                m.output %= b.motor.neurons.len();
            }
            for (slot, e) in b.edges.iter().enumerate() {
                for axis in e.kind.axes() {
                    if parent_muscles[e.child] >= e.kind.axes().len() * 2
                        || b.muscles
                            .iter()
                            .filter(|m| m.route == [Route::Child(slot)])
                            .count()
                            >= e.kind.axes().len() * 2
                    {
                        continue;
                    }
                    for sign in [-1.0, 1.0] {
                        let mut origin = e.attachment;
                        let tangent = (*axis + 1) % 3;
                        origin[tangent] =
                            (origin[tangent] + sign * limits.attachment_jitter).clamp(-1.0, 1.0);
                        // Keep at least one coordinate exactly on the bone surface.
                        let surface_axis = e
                            .attachment
                            .iter()
                            .position(|v| v.abs() == 1.0)
                            .unwrap_or(1);
                        origin[surface_axis] = e.attachment[surface_axis];
                        b.muscles.push(MuscleGene {
                            properties: MuscleProperties::default(),
                            route: vec![Route::Child(slot)],
                            origin,
                            insertion: [0.0, -1.0, 0.0],
                            area: 0.1,
                            output: usize::from(sign > 0.0) % b.motor.neurons.len(),
                        });
                    }
                }
            }
            b.muscles.truncate(128);
        }
        self.mutation_rates
            .iter_mut()
            .for_each(|r| *r = r.clamp(limits.mutation_floor, limits.mutation_ceiling));
    }
    fn copy_subtree(&mut self, donor: &Genome, root: usize) -> Option<usize> {
        let mut seen = BTreeSet::new();
        let mut pending = vec![root];
        while let Some(i) = pending.pop() {
            if seen.insert(i) {
                pending.extend(donor.bones[i].edges.iter().map(|e| e.child));
            }
        }
        if self.bones.len() + seen.len() > 64 {
            return None;
        }
        let map: BTreeMap<_, _> = seen
            .iter()
            .enumerate()
            .map(|(offset, id)| (*id, self.bones.len() + offset))
            .collect();
        for i in seen {
            let mut b = donor.bones[i].clone();
            for e in &mut b.edges {
                e.child = map[&e.child];
            }
            self.bones.push(b);
        }
        Some(map[&root])
    }
    fn remove_bone(&mut self, i: usize) {
        if i == self.root {
            return;
        }
        self.bones.remove(i);
        if self.root > i {
            self.root -= 1;
        }
        for b in &mut self.bones {
            b.edges.retain(|e| e.child != i);
            for e in &mut b.edges {
                if e.child > i {
                    e.child -= 1;
                }
            }
        }
    }
    pub fn mutate(
        &self,
        op: Operator,
        seed: u64,
        tick: u64,
        limits: &Limits,
    ) -> (Self, MutationRecord) {
        let mut g = self.clone();
        let mut r = crate::rng(seed, 0x4d555441, op as u64, tick);
        let bi = r.gen_range(0..g.bones.len());
        let jitter = r.gen_range(-0.15f32..0.15);
        let ethology_mutation = matches!(
            op,
            Operator::AddNeuron
                | Operator::RemoveNeuron
                | Operator::Weights
                | Operator::Activation
                | Operator::Plasticity
        ) && r.gen_bool(0.5);
        if ethology_mutation {
            std::mem::swap(&mut g.bones[bi].motor, &mut g.ethology);
        }
        match op {
            Operator::Parameter => {
                let axis = r.gen_range(0..3);
                let b = &mut g.bones[bi];
                match r.gen_range(0..12) {
                    0 => {
                        b.size[axis] = (b.size[axis] * (1.0 + jitter)).clamp(limits.min_size, 10.0)
                    }
                    1 => b.density = (b.density * (1.0 + jitter)).clamp(0.01, 100.0),
                    2 => b.color[axis] = (b.color[axis] + jitter).clamp(0.0, 1.0),
                    3 => b.motor.frequency = (b.motor.frequency + jitter).clamp(0.01, 10.0),
                    4 => b.motor.phase += jitter,
                    5 => {
                        b.motor.neurons[0].bias =
                            (b.motor.neurons[0].bias + jitter).clamp(-16.0, 16.0)
                    }
                    6 => {
                        if let Some(e) = b.edges.last_mut() {
                            e.scale = (e.scale + jitter).clamp(0.25, 2.0);
                        }
                    }
                    7 => {
                        if let Some(e) = b.edges.last_mut() {
                            e.orientation[axis] += jitter;
                        }
                    }
                    8 => {
                        if let Some(e) = b.edges.last_mut() {
                            e.limits[axis][1] = (e.limits[axis][1] + jitter)
                                .clamp(e.limits[axis][0], std::f32::consts::PI);
                        }
                    }
                    9 => {
                        if let Some(m) = b.muscles.last_mut() {
                            m.properties.contraction_scale =
                                (m.properties.contraction_scale + jitter).clamp(0.1, 2.0);
                        }
                    }
                    10 => {
                        if let Some(m) = b.muscles.last_mut() {
                            m.properties.stiffness_scale =
                                (m.properties.stiffness_scale + jitter).clamp(0.1, 2.0);
                        }
                    }
                    _ => {
                        if let Some(m) = b.muscles.last_mut() {
                            m.properties.damping_scale =
                                (m.properties.damping_scale + jitter).clamp(0.1, 2.0);
                        }
                    }
                }
                let k = r.gen_range(0..g.mutation_rates.len());
                g.mutation_rates[k] = (g.mutation_rates[k] + jitter * 0.1)
                    .clamp(limits.mutation_floor, limits.mutation_ceiling);
            }
            Operator::AddBone if g.bones.len() < 64 && g.bones[bi].edges.len() < 8 => {
                let child = g.bones.len();
                let mut bone = g.bones[bi].clone();
                bone.edges.clear();
                bone.muscles.clear();
                bone.size = bone.size.map(|x| (x * 0.65).max(limits.min_size));
                g.bones.push(bone);
                g.bones[bi].edges.push(Edge {
                    child,
                    attachment: [1.0, 0.0, 0.0],
                    child_attachment: [-1.0, 0.0, 0.0],
                    orientation: [0.0; 3],
                    scale: 1.0,
                    kind: JointKind::Hinge,
                    limits: [[-0.7, 0.7]; 3],
                    reflect: false,
                    terminal_only: false,
                });
            }
            Operator::RemoveBone => g.remove_bone(bi),
            Operator::AddJoint if g.bones[bi].edges.len() < 8 => {
                let child = r.gen_range(0..g.bones.len());
                g.bones[bi].edges.push(Edge {
                    child,
                    attachment: [0.0, 0.0, 1.0],
                    child_attachment: [0.0, 0.0, -1.0],
                    orientation: [0.0; 3],
                    scale: 0.7,
                    kind: JointKind::Hinge,
                    limits: [[-0.7, 0.7]; 3],
                    reflect: false,
                    terminal_only: false,
                });
            }
            Operator::RemoveJoint | Operator::PruneSubtree => {
                g.bones[bi].edges.pop();
            }
            Operator::JointType => {
                if let Some(e) = g.bones[bi].edges.last_mut() {
                    e.kind = [
                        JointKind::Fixed,
                        JointKind::Hinge,
                        JointKind::Universal,
                        JointKind::Ball,
                    ][r.gen_range(0..4)];
                }
            }
            Operator::Recursion => g.bones[bi].recursion_limit = r.gen_range(1..=12),
            Operator::Reflection => {
                if let Some(e) = g.bones[bi].edges.last_mut() {
                    e.reflect = !e.reflect;
                }
            }
            Operator::DuplicateSubtree => {
                if let Some(edge) = g.bones[bi].edges.last().cloned() {
                    if g.bones[bi].edges.len() < 8 {
                        if let Some(child) = g.copy_subtree(self, edge.child) {
                            let mut new = edge;
                            new.child = child;
                            new.reflect = !new.reflect;
                            new.attachment[2] = -new.attachment[2];
                            g.bones[bi].edges.push(new);
                        }
                    }
                }
            }
            Operator::AddMuscle => {
                if !g.bones[bi].edges.is_empty() && g.bones[bi].muscles.len() < 128 {
                    g.bones[bi].muscles.push(MuscleGene {
                        properties: MuscleProperties::default(),
                        route: vec![Route::Child(0)],
                        origin: [1.0, 0.3, 0.0],
                        insertion: [0.0, -1.0, 0.0],
                        area: 0.1,
                        output: 0,
                    });
                }
            }
            Operator::RemoveMuscle => {
                g.bones[bi].muscles.pop();
            }
            Operator::MoveAttachment => {
                if let Some(m) = g.bones[bi].muscles.last_mut() {
                    m.origin = [1.0, jitter, 0.2];
                }
            }
            Operator::MuscleArea => {
                if let Some(m) = g.bones[bi].muscles.last_mut() {
                    m.area = (m.area * (1.0 + jitter)).clamp(0.001, 10.0);
                }
            }
            Operator::MuscleSpan => {
                if let Some(m) = g.bones[bi].muscles.last_mut() {
                    if m.route.len() < limits.max_span {
                        m.route.push(Route::Child(0));
                    } else {
                        m.route.pop();
                    }
                }
            }
            Operator::AddSensor => {
                if g.bones[bi].sensors.len() < 32 {
                    g.bones[bi].sensors.push(Sensor::Gravity(r.gen_range(0..3)));
                }
            }
            Operator::RemoveSensor => {
                g.bones[bi].sensors.pop();
            }
            Operator::AddNeuron => {
                let b = &mut g.bones[bi].motor;
                if b.neurons.len() < 32 {
                    b.neurons.push(Neuron {
                        bias: jitter,
                        activation: Activation::Tanh,
                        inputs: vec![Connection {
                            input: Input::Local(0),
                            weight: 0.5,
                            learning_rate: 0.0,
                            decay: 0.0,
                        }],
                    });
                }
            }
            Operator::RemoveNeuron => g.bones[bi].motor.remove_neuron(),
            Operator::Weights => {
                let b = &mut g.bones[bi].motor;
                for n in &mut b.neurons {
                    for c in &mut n.inputs {
                        c.weight = (c.weight + jitter).clamp(-16.0, 16.0);
                    }
                }
            }
            Operator::Activation => {
                g.bones[bi].motor.neurons[0].activation = [
                    Activation::Tanh,
                    Activation::Sigmoid,
                    Activation::Sin,
                    Activation::Abs,
                    Activation::Threshold,
                ][r.gen_range(0..5)]
            }
            Operator::Plasticity => {
                if let Some(c) = g.bones[bi].motor.neurons[0].inputs.first_mut() {
                    c.learning_rate = if c.learning_rate > 0.0 { 0.0 } else { 0.01 };
                    c.decay = 0.1;
                }
            }
            Operator::LifeHistory => {
                let k = r.gen_range(0..4);
                let l = &mut g.life;
                match r.gen_range(0..12) {
                    0 => l.plant_efficiency = (l.plant_efficiency + jitter).clamp(0.0, 1.0),
                    1 => l.species_marker[k] = (l.species_marker[k] + jitter).clamp(0.0, 1.0),
                    2 => l.mate_weights[k] = (l.mate_weights[k] + jitter).clamp(-1.0, 1.0),
                    3 => {
                        l.offspring_fraction =
                            (l.offspring_fraction + jitter * 0.2).clamp(0.01, 0.5)
                    }
                    4 => l.immune_investment = (l.immune_investment + jitter).clamp(0.0, 1.0),
                    5 => l.resistance[k] = (l.resistance[k] + jitter).clamp(0.0, 1.0),
                    6 => l.pheromone_channel = r.gen_range(0..4),
                    7 => l.pheromone_strength = (l.pheromone_strength + jitter).clamp(0.0, 1.0),
                    8 => l.clutch_size = r.gen_range(1..=16),
                    9 => {
                        l.maturity =
                            ((l.maturity as f32 * (1.0 + jitter)) as u64).clamp(1, l.senescence - 1)
                    }
                    10 => {
                        l.senescence =
                            ((l.senescence as f32 * (1.0 + jitter)) as u64).max(l.maturity + 1)
                    }
                    _ => g.ethology.frequency = (g.ethology.frequency + jitter).clamp(0.01, 10.0),
                }
            }
            _ => {}
        }
        if ethology_mutation {
            std::mem::swap(&mut g.bones[bi].motor, &mut g.ethology);
        }
        g.repair(limits);
        if g.expand(limits).is_err() {
            g = self.clone();
        }
        let record = MutationRecord {
            operator: op.name().into(),
            parent: self.hash(),
            child: g.hash(),
            traits: None,
            viability: None,
            born: false,
            lifespan: None,
            lineage_survival: None,
        };
        (g, record)
    }
    pub fn crossover(&self, other: &Self, seed: u64, limits: &Limits) -> Self {
        if self == other {
            return self.clone();
        }
        let mut g = self.clone();
        let mut r = crate::rng(seed, 0x43524f53, 0, 0);
        let bi = r.gen_range(0..g.bones.len());
        if !g.bones[bi].edges.is_empty() {
            let ei = r.gen_range(0..g.bones[bi].edges.len());
            let old = g.bones[bi].edges[ei].child;
            let candidates: Vec<_> = other
                .bones
                .iter()
                .enumerate()
                .filter(|(_, b)| {
                    b.size
                        .iter()
                        .zip(g.bones[old].size)
                        .all(|(a, b)| (*a / b >= 0.5) && (*a / b <= 2.0))
                })
                .map(|(i, _)| i)
                .collect();
            if !candidates.is_empty() {
                let donor = candidates[r.gen_range(0..candidates.len())];
                if let Some(child) = g.copy_subtree(other, donor) {
                    g.bones[bi].edges[ei].child = child;
                    g.bones[bi]
                        .muscles
                        .retain(|m| !m.route.starts_with(&[Route::Child(ei)]));
                    g.bones[child]
                        .muscles
                        .retain(|m| !m.route.starts_with(&[Route::Parent]));
                }
            }
        }
        g.life.plant_efficiency = (self.life.plant_efficiency + other.life.plant_efficiency) * 0.5;
        g.life.offspring_fraction =
            (self.life.offspring_fraction + other.life.offspring_fraction) * 0.5;
        g.life.immune_investment =
            (self.life.immune_investment + other.life.immune_investment) * 0.5;
        for i in 0..4 {
            g.life.species_marker[i] =
                (self.life.species_marker[i] + other.life.species_marker[i]) * 0.5;
        }
        g.repair(limits);
        if g.expand(limits).is_ok() {
            g
        } else {
            self.clone()
        }
    }
    /// Graph/marker/trait distance plus exact identity separation for otherwise neutral alleles.
    pub fn compatibility(&self, other: &Self, traits: &[f32], other_traits: &[f32]) -> f32 {
        if self == other && traits == other_traits {
            return 0.0;
        }
        let count = (self.bones.len() as f32 - other.bones.len() as f32).abs() / 32.0;
        let joints = |g: &Self| g.bones.iter().map(|b| b.edges.len()).sum::<usize>();
        let topology = (joints(self) as f32 - joints(other) as f32).abs() / 64.0;
        let marker = self
            .life
            .species_marker
            .iter()
            .zip(other.life.species_marker)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / 4.0;
        let trait_distance = traits
            .iter()
            .zip(other_traits)
            .map(|(a, b)| (a - b).abs() / (1.0 + a.abs() + b.abs()))
            .sum::<f32>()
            / traits.len().max(other_traits.len()).max(1) as f32;
        count + topology + marker + trait_distance + 0.000001
    }
}
