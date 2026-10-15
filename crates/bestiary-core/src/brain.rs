//! Synchronous recurrent networks with bounded, heritable local plasticity.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum Activation {
    Tanh,
    Sigmoid,
    Sin,
    Abs,
    Threshold,
}
impl Activation {
    pub fn apply(self, x: f32) -> f32 {
        match self {
            Self::Tanh => libm::tanhf(x),
            Self::Sigmoid => 1.0 / (1.0 + libm::expf(-x)),
            Self::Sin => libm::sinf(x),
            Self::Abs => x.abs().min(1.0),
            Self::Threshold => {
                if x > 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Input {
    Sensor(usize),
    Local(usize),
    Parent(usize),
    Children(usize),
    Clock,
    Oscillator,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Connection {
    pub input: Input,
    pub weight: f32,
    pub learning_rate: f32,
    pub decay: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Neuron {
    pub bias: f32,
    pub activation: Activation,
    pub inputs: Vec<Connection>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Brain {
    pub neurons: Vec<Neuron>,
    pub frequency: f32,
    pub phase: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BrainState {
    pub values: Vec<f32>,
    pub weights: Vec<Vec<f32>>,
}
impl Brain {
    pub fn oscillator(phase: f32) -> Self {
        Self {
            frequency: 1.5,
            phase,
            neurons: [4.0, -4.0]
                .into_iter()
                .map(|weight| Neuron {
                    bias: 0.0,
                    activation: Activation::Sigmoid,
                    inputs: vec![
                        Connection {
                            input: Input::Oscillator,
                            weight,
                            learning_rate: 0.0,
                            decay: 0.0,
                        },
                        Connection {
                            input: Input::Local(0),
                            weight: 0.0,
                            learning_rate: 0.0,
                            decay: 0.0,
                        },
                        Connection {
                            input: Input::Parent(0),
                            weight: 0.0,
                            learning_rate: 0.0,
                            decay: 0.0,
                        },
                        Connection {
                            input: Input::Children(0),
                            weight: 0.0,
                            learning_rate: 0.0,
                            decay: 0.0,
                        },
                        Connection {
                            input: Input::Sensor(0),
                            weight: 0.0,
                            learning_rate: 0.0,
                            decay: 0.0,
                        },
                    ],
                })
                .collect(),
        }
    }
    pub fn state(&self) -> BrainState {
        BrainState {
            values: vec![0.0; self.neurons.len()],
            weights: self
                .neurons
                .iter()
                .map(|n| n.inputs.iter().map(|c| c.weight).collect())
                .collect(),
        }
    }
    pub fn valid(&self) -> bool {
        self.frequency.is_finite()
            && self.frequency >= 0.0
            && self.phase.is_finite()
            && !self.neurons.is_empty()
            && self.neurons.len() <= 32
            && self.neurons.iter().all(|n| {
                n.bias.is_finite()
                    && n.inputs.len() <= 64
                    && n.inputs.iter().all(|c| {
                        c.weight.is_finite()
                            && c.weight.abs() <= 16.0
                            && c.learning_rate.is_finite()
                            && (0.0..=1.0).contains(&c.learning_rate)
                            && c.decay.is_finite()
                            && (0.0..=1.0).contains(&c.decay)
                            && match c.input {
                                Input::Local(i) => i < self.neurons.len(),
                                _ => true,
                            }
                    })
            })
    }
    /// Every input uses the previous tick's state, including parent/child pools.
    pub fn step(
        &self,
        state: &mut BrainState,
        sensors: &[f32],
        parent: &[f32],
        children: &[f32],
        time: f32,
    ) {
        let get = |v: &[f32], i| v.get(i).copied().unwrap_or(0.0);
        let mut next = Vec::with_capacity(self.neurons.len());
        for (ni, n) in self.neurons.iter().enumerate() {
            let mut sum = n.bias;
            for (ci, c) in n.inputs.iter().enumerate() {
                let input = match c.input {
                    Input::Sensor(i) => get(sensors, i),
                    Input::Local(i) => get(&state.values, i),
                    Input::Parent(i) => get(parent, i),
                    Input::Children(i) => get(children, i),
                    Input::Clock => time,
                    Input::Oscillator => {
                        libm::sinf(time * std::f32::consts::TAU * self.frequency + self.phase)
                    }
                };
                sum += state.weights[ni][ci] * input;
                state.weights[ni][ci] = (state.weights[ni][ci]
                    + c.learning_rate
                        * (input * state.values[ni]
                            - c.decay * (state.weights[ni][ci] - c.weight)))
                    .clamp(-16.0, 16.0);
            }
            next.push(n.activation.apply(sum));
        }
        state.values = next;
    }
    pub fn remove_neuron(&mut self) {
        if self.neurons.len() > 1 {
            self.neurons.pop();
        }
        let len = self.neurons.len();
        for n in &mut self.neurons {
            n.inputs
                .retain(|c| !matches!(c.input, Input::Local(i) if i>=len));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recurrent_plasticity_is_repeatable_bounded_and_uses_old_values() {
        let mut b = Brain::oscillator(0.0);
        b.neurons[0].inputs[0].learning_rate = 0.1;
        let mut a = b.state();
        let mut c = b.state();
        for t in 0..1000 {
            b.step(&mut a, &[], &[], &[], t as f32 / 240.0);
            b.step(&mut c, &[], &[], &[], t as f32 / 240.0);
        }
        assert_eq!(a, c);
        assert!(a.weights[0][0].abs() <= 16.0);
        assert_ne!(a.weights[0][0], 4.0);
        for f in [
            Activation::Tanh,
            Activation::Sigmoid,
            Activation::Sin,
            Activation::Abs,
            Activation::Threshold,
        ] {
            assert!(f.apply(2.0).is_finite());
        }
    }
    #[test]
    fn invalid_neural_genes_are_rejected() {
        let mut b = Brain::oscillator(0.0);
        b.frequency = f32::NAN;
        assert!(!b.valid());
        b.frequency = 1.0;
        b.neurons[0].inputs[0].input = Input::Local(99);
        assert!(!b.valid());
    }
}
