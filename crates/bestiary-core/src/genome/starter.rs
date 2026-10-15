//! Authored founder genes. The runtime only executes ordinary inherited brains.
use super::*;
use crate::brain::{Activation, Connection, Input, Neuron};
use std::sync::OnceLock;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pool {
    frequency: f32,
    phase_offset: f32,
    biases: [f32; 2],
    oscillator: [f32; 2],
    proprioception: [f32; 2],
    angular_velocity: [f32; 2],
    contact: [f32; 2],
}
impl Pool {
    fn brain(&self, phase: f32) -> Brain {
        Brain {
            frequency: self.frequency,
            phase: phase + self.phase_offset,
            neurons: (0..2)
                .map(|i| Neuron {
                    bias: self.biases[i],
                    activation: Activation::Sigmoid,
                    inputs: [
                        (Input::Oscillator, self.oscillator[i]),
                        (Input::Sensor(0), self.proprioception[i]),
                        (Input::Sensor(1), self.angular_velocity[i]),
                        (Input::Sensor(2), self.contact[i]),
                    ]
                    .into_iter()
                    .map(|(input, weight)| Connection {
                        input,
                        weight,
                        learning_rate: 0.0,
                        decay: 0.0,
                    })
                    .collect(),
                })
                .collect(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Founder {
    upper_shape: Shape,
    upper_size: [f32; 3],
    lower_shape: Shape,
    lower_size: [f32; 3],
    hip_rest: f32,
    knee_rest: f32,
    hip_limits: [f32; 2],
    knee_limits: [f32; 2],
    hip_area: f32,
    knee_area: f32,
    knee_origin: [f32; 3],
    knee_insertion: [f32; 3],
    hip: Pool,
    knee: Pool,
}
impl Founder {
    fn validate(&self) -> Result<(), String> {
        if self
            .upper_size
            .iter()
            .chain(&self.lower_size)
            .any(|x| !x.is_finite() || *x < 0.02 || *x > 10.0)
            || ![self.hip_rest, self.knee_rest]
                .iter()
                .all(|x| x.is_finite())
            || [self.hip_limits, self.knee_limits].iter().any(|v| {
                !v[0].is_finite()
                    || !v[1].is_finite()
                    || v[0] > 0.0
                    || v[1] < 0.0
                    || v[0] < -std::f32::consts::PI
                    || v[1] > std::f32::consts::PI
            })
            || [self.hip_area, self.knee_area]
                .iter()
                .any(|x| !x.is_finite() || !(0.001..=10.0).contains(x))
            || !surface(&self.knee_origin)
            || !surface(&self.knee_insertion)
            || !self.hip.brain(0.0).valid()
            || !self.knee.brain(0.0).valid()
            || self.hip.frequency <= 0.0
            || self.knee.frequency <= 0.0
        {
            Err("invalid authored founder genes".into())
        } else {
            Ok(())
        }
    }
    fn build(&self, limits: &Limits) -> Genome {
        let mut g = Genome::rigid_limb_fixture(limits);
        let hips: Vec<_> = g.bones[0].edges.iter().map(|e| e.child).collect();
        for (slot, id) in hips.into_iter().enumerate() {
            let phase = g.bones[id].motor.phase;
            g.bones[id].shape = self.upper_shape;
            g.bones[id].size = self.upper_size;
            g.bones[id].motor = self.hip.brain(phase);
            for m in &mut g.bones[id].muscles {
                m.area = self.hip_area;
            }
            let mut shin = g.bones[id].clone();
            g.bones[id].sensors[0] = Sensor::Gravity(0);
            shin.shape = self.lower_shape;
            shin.size = self.lower_size;
            shin.sensors[2] = Sensor::TerrainContact;
            shin.motor = self.knee.brain(phase);
            for (i, m) in shin.muscles.iter_mut().enumerate() {
                let sign = if i == 0 { -1.0 } else { 1.0 };
                m.area = self.knee_area;
                m.origin = self.knee_origin;
                m.origin[0] *= sign;
                m.insertion = self.knee_insertion;
                m.insertion[0] *= sign;
            }
            let child = g.bones.len();
            g.bones.push(shin);
            g.bones[id].edges = vec![Edge {
                child,
                attachment: [0.0, -1.0, 0.0],
                child_attachment: [0.0, 1.0, 0.0],
                orientation: [0.0, 0.0, self.knee_rest],
                scale: 1.0,
                kind: JointKind::Hinge,
                limits: [self.knee_limits; 3],
                reflect: false,
                terminal_only: false,
            }];
            g.bones[0].edges[slot].orientation = [0.0, 0.0, self.hip_rest];
            g.bones[0].edges[slot].limits = [self.hip_limits; 3];
        }
        g.repair(limits);
        g
    }
}
impl Genome {
    /// Inheritable hips, knees and terrain-responsive motor pools from the
    /// documented authored template. Saved genomes retain these values unchanged.
    pub fn starter(limits: &Limits) -> Self {
        static CONFIG: OnceLock<Founder> = OnceLock::new();
        CONFIG
            .get_or_init(|| {
                let c: Founder = ron::from_str(include_str!("../../../../config/starter.ron"))
                    .expect("bundled founder parses");
                c.validate().expect("bundled founder validates");
                c
            })
            .build(limits)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coordinated_walking_genes_inherit_two_tripods_and_actuated_knees() {
        #[derive(Deserialize)]
        struct Config {
            genome: Limits,
        }
        let limits = ron::from_str::<Config>(include_str!("../../../../config/world.ron"))
            .unwrap()
            .genome;
        let g = Genome::starter(&limits);
        let body = g.expand(&limits).unwrap();
        assert_eq!(body.parts.len(), 13);
        assert_eq!(g.bones[0].edges.len(), 6);
        assert!(body.actuated_axes_covered());
        let mut tripods = [0; 2];
        for edge in &g.bones[0].edges {
            let hip = &g.bones[edge.child];
            let knee = &g.bones[hip.edges[0].child];
            tripods[usize::from(hip.motor.phase > 0.0)] += 1;
            assert_eq!(hip.motor.frequency, knee.motor.frequency);
            assert!(
                (knee.motor.phase - hip.motor.phase - std::f32::consts::FRAC_PI_2).abs() < 1e-6
            );
            assert!(matches!(hip.sensors[0], Sensor::Gravity(0)));
            assert!(matches!(knee.sensors[2], Sensor::TerrainContact));
            assert!(knee
                .motor
                .neurons
                .iter()
                .all(|n| n
                    .inputs
                    .iter()
                    .any(|c| matches!(c.input, Input::Sensor(2)) && c.weight != 0.0)));
            assert_eq!(hip.muscles.len(), 2);
            assert_eq!(knee.muscles.len(), 2);
        }
        assert_eq!(tripods, [3, 3]);
        assert_eq!(Genome::from_share(&g.to_share(), &limits).unwrap(), g);
        assert_eq!(
            benchmark_genome(&limits)
                .expand(&limits)
                .unwrap()
                .parts
                .len(),
            12
        );
    }
    #[test]
    fn founder_configuration_rejects_invalid_geometry_and_neural_weights() {
        let c: Founder = ron::from_str(include_str!("../../../../config/starter.ron")).unwrap();
        c.validate().unwrap();
        let mut bad = c.clone();
        bad.lower_size[0] = -1.0;
        assert!(bad.validate().is_err());
        let mut bad = c.clone();
        bad.knee_origin = [0.0; 3];
        assert!(bad.validate().is_err());
        let mut bad = c.clone();
        bad.knee_limits = [0.1, 0.4];
        assert!(bad.validate().is_err());
        let mut bad = c.clone();
        bad.knee.frequency = 0.0;
        assert!(bad.validate().is_err());
        let mut bad = c.clone();
        bad.hip.oscillator[0] = f32::NAN;
        assert!(bad.validate().is_err());
        let mut bad = c;
        bad.hip_area = 0.0;
        assert!(bad.validate().is_err());
        assert!(ron::from_str::<Founder>("(unknown: 1)").is_err());
    }
}
