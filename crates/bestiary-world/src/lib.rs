//! Fixed-tick trait ecology. Rendering and asynchronous execution are forbidden here.
use bestiary_core::{
    brain::{Activation, Brain, BrainState, Connection, Input, Neuron},
    genome::Genome,
};
use bestiary_sim::{
    combine::{evaluate, Traits},
    config::Config,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plant {
    pub energy: i64,
    pub nutrients: i64,
    pub regrowth: f32,
    pub toughness: f32,
    pub toxicity: f32,
    pub dispersal: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cell {
    pub height: f32,
    pub temperature: f32,
    pub moisture: f32,
    pub soil: i64,
    pub plant: Plant,
    pub pheromones: [f32; 4],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LineageGenome {
    pub genome: Genome,
    pub traits: Traits,
    pub viability: f32,
    pub discovered: u64,
    pub parents: Vec<[u8; 32]>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Agent {
    pub id: u64,
    pub genome: [u8; 32],
    pub position: [f32; 2],
    pub energy: i64,
    pub nutrients: i64,
    pub age: u64,
    pub health: f32,
    pub growth: f32,
    pub injury: f32,
    pub brain: BrainState,
    pub outputs: Vec<f32>,
    pub species: u64,
    pub parents: Vec<u64>,
    pub offspring: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Carcass {
    pub position: [f32; 2],
    pub energy: i64,
    pub nutrients: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct World {
    pub seed: u64,
    pub tick: u64,
    pub config: Config,
    pub cells: Vec<Cell>,
    pub agents: Vec<Agent>,
    pub genomes: BTreeMap<[u8; 32], LineageGenome>,
    pub carcasses: Vec<Carcass>,
    pub next_id: u64,
    pub solar_input: i64,
    pub dissipated: i64,
    pub initial_energy: i64,
    pub initial_nutrients: i64,
}
/// All rule execution passes through this interface, leaving phase-two hooks additive.
pub trait WorldRules {
    fn step(&self, world: &mut World);
}
pub struct NaturalRules;
impl WorldRules for NaturalRules {
    fn step(&self, world: &mut World) {
        world.natural_step();
    }
}

pub fn forager_brain() -> Brain {
    let mut neurons = vec![];
    for i in 0..13 {
        let (bias, input, weight, activation) = match i {
            0 => (0.0, Input::Oscillator, 1.0, Activation::Tanh),
            1 => (0.3, Input::Local(0), 0.4, Activation::Tanh),
            2 => (-2.0, Input::Sensor(4), -4.0, Activation::Sigmoid),
            3 => (4.0, Input::Sensor(0), -2.0, Activation::Sigmoid),
            7 => (-2.0, Input::Sensor(0), 5.0, Activation::Sigmoid),
            _ => (-4.0, Input::Sensor(2), 0.0, Activation::Sigmoid),
        };
        neurons.push(Neuron {
            bias,
            activation,
            inputs: vec![Connection {
                input,
                weight,
                learning_rate: 0.0,
                decay: 0.0,
            }],
        });
    }
    Brain {
        neurons,
        frequency: 0.001,
        phase: 0.0,
    }
}
fn take(value: &mut i64, amount: i64) -> i64 {
    let n = amount.max(0).min(*value);
    *value -= n;
    n
}
fn fraction(value: i64, f: f32) -> i64 {
    ((value as f64 * f64::from(f.clamp(0.0, 1.0))) as i64).clamp(0, value)
}
impl World {
    pub fn new(seed: u64, config: Config, population: usize) -> Result<Self, String> {
        let mut g = Genome::starter(&config.genome);
        g.ethology = forager_brain();
        Self::with_founder(seed, config, population, g)
    }
    pub fn with_founder(
        seed: u64,
        config: Config,
        population: usize,
        genome: Genome,
    ) -> Result<Self, String> {
        config.world.validate()?;
        if population > config.world.population_limit {
            return Err("initial population exceeds configured bound".into());
        }
        let evaluated = evaluate(&genome, &config)?;
        if evaluated.exploit_suspect {
            return Err("founder failed the energy audit".into());
        }
        let hash = genome.hash();
        let cfg = &config.world;
        let mut cells = vec![];
        for i in 0..cfg.grid * cfg.grid {
            let mut rng = bestiary_core::rng(seed, 0x54455252, i as u64, 0);
            let x = (i % cfg.grid) as f32 / cfg.grid as f32;
            let z = (i / cfg.grid) as f32 / cfg.grid as f32;
            let height = libm::sinf(x * std::f32::consts::TAU) * 0.3
                + libm::cosf(z * std::f32::consts::TAU) * 0.3
                + rng.gen_range(-0.1..0.1);
            cells.push(Cell {
                height,
                temperature: 1.0 - (z - 0.5).abs() - height.max(0.0) * 0.2,
                moisture: rng.gen_range(0.5..1.0),
                soil: cfg.soil_per_cell,
                plant: Plant {
                    energy: cfg.plant_energy,
                    nutrients: cfg.plant_nutrients,
                    regrowth: rng.gen_range(0.5..1.0),
                    toughness: 0.1,
                    toxicity: 0.02,
                    dispersal: 1,
                },
                pheromones: [0.0; 4],
            });
        }
        let mut agents = vec![];
        let land: Vec<_> = cells
            .iter()
            .enumerate()
            .filter(|(_, c)| c.height > cfg.water_level)
            .map(|(i, _)| i)
            .collect();
        if land.is_empty() && population > 0 {
            return Err("no land for founding population".into());
        }
        for id in 0..population {
            let mut rng = bestiary_core::rng(seed, 0x42495254, id as u64, 0);
            let cell = land[rng.gen_range(0..land.len())];
            agents.push(Agent {
                id: id as u64,
                genome: hash,
                position: [
                    ((cell % cfg.grid) as f32 + 0.5) * cfg.cell_metres,
                    ((cell / cfg.grid) as f32 + 0.5) * cfg.cell_metres,
                ],
                energy: cfg.initial_energy,
                nutrients: (evaluated.traits.mass * cfg.nutrient_units_per_kg) as i64,
                age: 0,
                health: 1.0,
                growth: 1.0,
                injury: 0.0,
                brain: genome.ethology.state(),
                outputs: vec![0.0; 13],
                species: 0,
                parents: vec![],
                offspring: 0,
            });
        }
        let mut genomes = BTreeMap::new();
        genomes.insert(
            hash,
            LineageGenome {
                genome,
                traits: evaluated.traits,
                viability: evaluated.viability,
                discovered: 0,
                parents: vec![],
            },
        );
        let mut w = Self {
            seed,
            tick: 0,
            config,
            cells,
            agents,
            genomes,
            carcasses: vec![],
            next_id: population as u64,
            solar_input: 0,
            dissipated: 0,
            initial_energy: 0,
            initial_nutrients: 0,
        };
        let (energy, nutrients) = w.totals();
        w.initial_energy = energy;
        w.initial_nutrients = nutrients;
        Ok(w)
    }
    pub fn cell_at(&self, p: [f32; 2]) -> usize {
        cell_at(p, &self.config.world)
    }
    pub fn totals(&self) -> (i64, i64) {
        let energy = self.cells.iter().map(|c| c.plant.energy).sum::<i64>()
            + self.agents.iter().map(|a| a.energy).sum::<i64>()
            + self.carcasses.iter().map(|c| c.energy).sum::<i64>();
        let nutrients = self
            .cells
            .iter()
            .map(|c| c.soil + c.plant.nutrients)
            .sum::<i64>()
            + self.agents.iter().map(|a| a.nutrients).sum::<i64>()
            + self.carcasses.iter().map(|c| c.nutrients).sum::<i64>();
        (energy, nutrients)
    }
    pub fn conserved(&self) -> bool {
        let (e, n) = self.totals();
        e + self.dissipated == self.initial_energy + self.solar_input && n == self.initial_nutrients
    }
    pub fn state_hash(&self) -> [u8; 32] {
        bestiary_core::digest(self).expect("world serializes")
    }
    pub fn step(&mut self) {
        NaturalRules.step(self)
    }
    pub fn step_with(&mut self, rules: &dyn WorldRules) {
        rules.step(self)
    }
    fn natural_step(&mut self) {
        let cfg = &self.config.world;
        let seasonal = libm::sinf(
            std::f32::consts::TAU * (self.tick % cfg.season_ticks) as f32 / cfg.season_ticks as f32,
        );
        let light = (1.0 + cfg.season_amplitude * seasonal).max(0.0);
        for cell in &mut self.cells {
            let capacity = cfg.plant_energy;
            let temperature = (cell.temperature + cfg.season_amplitude * seasonal).clamp(0.0, 2.0);
            let growth = (cfg.solar_per_cell as f64
                * f64::from(light * temperature * cell.moisture * cell.plant.regrowth))
                as i64;
            let solar = growth.min((capacity - cell.plant.energy).max(0));
            // Growth requires a physical nutrient transfer; no energy enters without biomass growth.
            let nutrient_need = (solar / 1000).max(0);
            let nutrient = take(&mut cell.soil, nutrient_need);
            let accepted = solar.min(nutrient.saturating_mul(1000));
            cell.plant.energy += accepted;
            cell.plant.nutrients += nutrient;
            self.solar_input += accepted;
        }
        if self.tick % cfg.plant_epoch == 0 {
            let old = self
                .cells
                .iter()
                .map(|c| c.plant.clone())
                .collect::<Vec<_>>();
            for (i, cell) in self.cells.iter_mut().enumerate() {
                let mut rng = bestiary_core::rng(self.seed, 0x504c414e, i as u64, self.tick);
                let radius = i64::from(old[i].dispersal);
                let dx = rng.gen_range(-radius..=radius);
                let dz = rng.gen_range(-radius..=radius);
                let x = (i as i64 % cfg.grid as i64 + dx).rem_euclid(cfg.grid as i64) as usize;
                let z = (i as i64 / cfg.grid as i64 + dz).rem_euclid(cfg.grid as i64) as usize;
                let donor = &old[z * cfg.grid + x];
                if donor.energy > old[i].energy {
                    cell.plant.regrowth =
                        (donor.regrowth + rng.gen_range(-0.02..0.02)).clamp(0.1, 1.0);
                    cell.plant.toughness =
                        (donor.toughness + rng.gen_range(-0.02..0.02)).clamp(0.0, 1.0);
                    cell.plant.toxicity =
                        (donor.toxicity + rng.gen_range(-0.01..0.01)).clamp(0.0, 1.0);
                    cell.plant.dispersal =
                        (i16::from(donor.dispersal) + rng.gen_range(-1..=1)).clamp(1, 4) as u8;
                }
            }
        }
        let extent = cfg.grid as f32 * cfg.cell_metres;
        for agent in &mut self.agents {
            if agent.health <= 0.0 {
                continue;
            }
            let entry = &self.genomes[&agent.genome];
            let genes = &entry.genome;
            let traits = &entry.traits;
            let cell_id = cell_at(agent.position, cfg);
            let reserve = (traits.mass * agent.growth * cfg.reserve_per_kg as f32) as i64;
            if self.tick % cfg.decision_interval == agent.id % cfg.decision_interval {
                let cell = &self.cells[cell_id];
                let inputs = [
                    agent.energy as f32 / reserve.max(1) as f32,
                    agent.age as f32 / genes.life.senescence as f32,
                    agent.health,
                    agent.growth,
                    cell.plant.energy as f32 / cfg.plant_energy.max(1) as f32,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                    1.0,
                    0.0,
                    cell.temperature + cfg.season_amplitude * seasonal,
                    if cell.height < cfg.water_level {
                        1.0
                    } else {
                        0.0
                    },
                    (self.tick % 240) as f32 / 240.0,
                    seasonal,
                ];
                genes
                    .ethology
                    .step(&mut agent.brain, &inputs, &[], &[], self.tick as f32);
                agent.outputs = agent.brain.values.clone();
            }
            let output = |i: usize| agent.outputs.get(i).copied().unwrap_or(0.0);
            let effort = output(2).clamp(0.0, 1.0);
            let dir = [output(0), output(1)];
            let norm = libm::sqrtf(dir[0] * dir[0] + dir[1] * dir[1]).max(1.0);
            let speed = if self.cells[cell_id].height < cfg.water_level {
                traits.swim_speed
            } else {
                traits.speed
            };
            let distance =
                speed * effort * agent.growth * (1.0 - agent.injury) * cfg.seconds_per_tick;
            let moved = [dir[0] / norm * distance, dir[1] / norm * distance];
            agent.position = [
                (agent.position[0] + moved[0]).rem_euclid(extent),
                (agent.position[1] + moved[1]).rem_euclid(extent),
            ];
            let actual_distance = libm::sqrtf(moved[0] * moved[0] + moved[1] * moved[1]);
            let cell = &mut self.cells[cell_at(agent.position, cfg)];
            if output(3) > output(8) && agent.energy < reserve {
                let meal = take(
                    &mut cell.plant.energy,
                    cfg.bite_energy.min((reserve - agent.energy) * 2),
                );
                let before = cell.plant.energy + meal;
                let nutrient_amount = ((cell.plant.nutrients as i128 * meal as i128)
                    / i128::from(before.max(1))) as i64;
                let nutrient = take(&mut cell.plant.nutrients, nutrient_amount);
                let assimilated = fraction(
                    meal,
                    genes.life.plant_efficiency / (1.0 + cell.plant.toughness),
                );
                agent.energy += assimilated;
                self.dissipated += meal - assimilated;
                let needed =
                    (traits.mass * (1.0 - agent.growth) * cfg.nutrient_units_per_kg) as i64;
                let retained = nutrient
                    .min(needed.max(0))
                    .min((traits.mass * cfg.growth_per_tick * cfg.nutrient_units_per_kg) as i64);
                agent.nutrients += retained;
                cell.soil += nutrient - retained;
                agent.growth = (agent.growth
                    + retained as f32 / (traits.mass * cfg.nutrient_units_per_kg).max(1.0))
                .min(1.0);
                agent.health = (agent.health
                    - cell.plant.toxicity * (1.0 - genes.life.immune_investment) * 0.01)
                    .max(0.0);
            }
            let sensors = genes.bones.iter().map(|b| b.sensors.len()).sum::<usize>();
            let neurons = genes
                .bones
                .iter()
                .map(|b| b.motor.neurons.len())
                .sum::<usize>()
                + genes.ethology.neurons.len();
            let rate = traits.standing_cost * agent.growth
                + traits.mass * agent.growth * cfg.basal_per_kg
                + sensors as f32 * cfg.sensor_cost
                + neurons as f32 * cfg.brain_cost
                + genes.life.immune_investment;
            let expenditure = ((rate * cfg.seconds_per_tick
                + actual_distance * traits.energy_per_meter)
                * cfg.energy_units_per_joule) as i64;
            let paid = take(&mut agent.energy, expenditure);
            self.dissipated += paid;
            if paid < expenditure {
                agent.health -= cfg.starvation_damage;
            } else {
                agent.health = (agent.health + cfg.healing).min(1.0);
            }
            agent.age += 1;
            if agent.age > genes.life.senescence {
                agent.health -= 1.0 / (genes.life.senescence - genes.life.maturity).max(1) as f32;
            }
        }
        let mut living = Vec::with_capacity(self.agents.len());
        for agent in self.agents.drain(..) {
            if agent.health <= 0.0 {
                self.carcasses.push(Carcass {
                    position: agent.position,
                    energy: agent.energy,
                    nutrients: agent.nutrients,
                });
            } else {
                living.push(agent);
            }
        }
        self.agents = living;
        for carcass in &mut self.carcasses {
            let cell = cell_at(carcass.position, cfg);
            let n = fraction(carcass.nutrients, cfg.decay_fraction)
                .max(i64::from(carcass.nutrients > 0));
            carcass.nutrients -= n;
            self.cells[cell].soil += n;
            let e = fraction(carcass.energy, cfg.decay_fraction).max(i64::from(carcass.energy > 0));
            carcass.energy -= e;
            self.dissipated += e;
        }
        self.carcasses.retain(|c| c.energy > 0 || c.nutrients > 0);
        self.tick += 1;
    }
}
fn cell_at(p: [f32; 2], cfg: &bestiary_core::settings::WorldConfig) -> usize {
    let x = (p[0] / cfg.cell_metres) as usize % cfg.grid;
    let z = (p[1] / cfg.cell_metres) as usize % cfg.grid;
    z * cfg.grid + x
}
