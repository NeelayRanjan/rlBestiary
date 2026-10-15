use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldConfig {
    pub grid: usize,
    pub cell_metres: f32,
    pub seconds_per_tick: f32,
    pub decision_interval: u64,
    pub season_ticks: u64,
    pub plant_epoch: u64,
    pub solar_per_cell: i64,
    pub soil_per_cell: i64,
    pub plant_energy: i64,
    pub plant_nutrients: i64,
    pub initial_energy: i64,
    pub reserve_per_kg: i64,
    pub bite_energy: i64,
    pub basal_per_kg: f32,
    pub sensor_cost: f32,
    pub brain_cost: f32,
    pub growth_per_tick: f32,
    pub decay_fraction: f32,
    pub starvation_damage: f32,
    pub healing: f32,
    pub water_level: f32,
    pub terrain_frequency: f32,
    pub season_amplitude: f32,
    pub population_limit: usize,
    pub energy_units_per_joule: f32,
    pub nutrient_units_per_kg: f32,
}
impl WorldConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !(2..=256).contains(&self.grid)
            || self.population_limit == 0
            || self.population_limit > 100_000
            || self.decision_interval == 0
            || self.season_ticks == 0
            || self.plant_epoch == 0
            || [
                self.cell_metres,
                self.terrain_frequency,
                self.seconds_per_tick,
                self.reserve_per_kg as f32,
                self.energy_units_per_joule,
                self.nutrient_units_per_kg,
            ]
            .iter()
            .any(|x| !x.is_finite() || *x <= 0.0)
            || [
                self.basal_per_kg,
                self.sensor_cost,
                self.brain_cost,
                self.growth_per_tick,
                self.starvation_damage,
                self.healing,
                self.season_amplitude,
            ]
            .iter()
            .any(|x| !x.is_finite() || *x < 0.0)
            || [
                self.solar_per_cell,
                self.soil_per_cell,
                self.plant_energy,
                self.plant_nutrients,
                self.initial_energy,
                self.bite_energy,
            ]
            .iter()
            .any(|x| *x < 0 || *x > 1_000_000_000_000)
            || !self.decay_fraction.is_finite()
            || !(0.0..=1.0).contains(&self.decay_fraction)
            || !self.water_level.is_finite()
        {
            return Err("invalid world configuration".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EcologyConfig {
    pub attack_scale: f32,
    pub attack_cost: i64,
    pub injury_fraction: f32,
    pub refuge_fraction: f32,
    pub pathogen_count: usize,
    pub pathogen_epoch: u64,
    pub transmission: f32,
    pub virulence: f32,
    pub antigen_drift: f32,
    pub pheromone_diffusion: f32,
    pub pheromone_decay: f32,
    pub pheromone_cost: i64,
    pub tectonic_epoch: u64,
    pub uplift: f32,
    pub sea_amplitude: f32,
    pub shock_probability: f32,
}
impl EcologyConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.pathogen_count > 32
            || self.pathogen_epoch == 0
            || self.tectonic_epoch == 0
            || self.attack_cost < 0
            || self.pheromone_cost < 0
            || [self.attack_scale, self.uplift, self.sea_amplitude]
                .iter()
                .any(|x| !x.is_finite() || *x < 0.0)
            || [
                self.injury_fraction,
                self.refuge_fraction,
                self.transmission,
                self.virulence,
                self.antigen_drift,
                self.pheromone_diffusion,
                self.pheromone_decay,
                self.shock_probability,
            ]
            .iter()
            .any(|x| !x.is_finite() || !(0.0..=1.0).contains(x))
        {
            return Err("invalid ecology configuration".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReproductionConfig {
    pub epoch: u64,
    pub mate_radius: f32,
    pub compatibility_threshold: f32,
    pub energy_fraction: f32,
    pub gestation_loss: f32,
    pub mutation_probability: f32,
    pub founder_maturity: u64,
    pub founder_senescence: u64,
    pub species_epoch: u64,
    pub species_min_population: usize,
    pub species_min_duration: u64,
}
impl ReproductionConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.epoch == 0
            || self.species_epoch == 0
            || self.species_min_population == 0
            || self.founder_maturity == 0
            || self.founder_senescence <= self.founder_maturity
            || !self.mate_radius.is_finite()
            || self.mate_radius <= 0.0
            || !self.compatibility_threshold.is_finite()
            || self.compatibility_threshold <= 0.0
            || [
                self.energy_fraction,
                self.gestation_loss,
                self.mutation_probability,
            ]
            .iter()
            .any(|x| !x.is_finite() || !(0.0..=1.0).contains(x))
        {
            return Err("invalid reproduction configuration".into());
        }
        Ok(())
    }
}
