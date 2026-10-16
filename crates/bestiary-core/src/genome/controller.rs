//! Motor-controller-only variation for physics training. It is not an
//! `Operator`: adding one would change `Operator::ALL` and the genome encoding.
use super::Genome;
use crate::brain::{Activation, Brain};
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
    /// Perturbs motor brains only; body, ethology and life history are untouched.
    pub fn mutate_controller(
        &self,
        p: &ControllerMutation,
        seed: u64,
        entity: u64,
        tick: u64,
    ) -> Genome {
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
            motor.neurons[n].bias =
                (motor.neurons[n].bias + r.gen_range(-s..=s) * 4.0).clamp(-16.0, 16.0);
            if r.gen::<f32>() < p.activation_rate {
                let n = r.gen_range(0..motor.neurons.len());
                motor.neurons[n].activation = [
                    Activation::Tanh,
                    Activation::Sigmoid,
                    Activation::Sin,
                    Activation::Abs,
                    Activation::Threshold,
                ][r.gen_range(0..5)];
            }
        }
        g
    }

    /// True when every gene except the bones' motor brains is identical.
    pub fn same_morphology(&self, other: &Genome) -> bool {
        let strip = |g: &Genome| {
            let mut g = g.clone();
            for bone in &mut g.bones {
                bone.motor = Brain {
                    neurons: vec![],
                    frequency: 0.0,
                    phase: 0.0,
                };
            }
            g
        };
        strip(self) == strip(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Limits;
    fn limits() -> Limits {
        Limits {
            max_parts: 32,
            max_depth: 12,
            max_mass: 100.0,
            min_size: 0.02,
            max_span: 2,
            mutation_floor: 0.001,
            mutation_ceiling: 0.5,
            attachment_jitter: 0.2,
        }
    }
    fn params() -> ControllerMutation {
        ControllerMutation {
            weight_rate: 0.3,
            weight_sigma: 0.5,
            oscillator_sigma: 0.2,
            activation_rate: 0.1,
        }
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
        assert_eq!(
            g.mutate_controller(&params(), 7, 1, 3),
            g.mutate_controller(&params(), 7, 1, 3)
        );
        assert_ne!(
            g.mutate_controller(&params(), 7, 1, 3),
            g.mutate_controller(&params(), 7, 2, 3)
        );
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
