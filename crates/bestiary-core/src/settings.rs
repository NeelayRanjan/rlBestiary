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
