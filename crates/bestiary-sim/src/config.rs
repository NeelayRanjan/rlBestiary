use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub archive: bestiary_core::settings::ArchiveConfig,
    pub reproduction: bestiary_core::settings::ReproductionConfig,
    pub ecology: bestiary_core::settings::EcologyConfig,
    pub world: bestiary_core::settings::WorldConfig,
    pub combine: CombineConfig,
    pub genome: bestiary_core::genome::Limits,
    pub physics: PhysicsConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicsConfig {
    pub gravity: f32,
    pub substeps: u32,
    pub friction: f32,
    pub force_per_area: f32,
    pub length_tension_width: f32,
    pub max_contraction_fraction: f32,
    pub passive_stiffness: f32,
    pub damping: f32,
    pub isometric_cost: f32,
    pub audit_absolute_tolerance: f32,
    pub audit_relative_tolerance: f32,
    pub fixture_frequency: f32,
    pub fixture_muscle_area: f32,
    pub benchmark_trial_seconds: f32,
}

impl Config {
    pub fn parse(source: &str) -> Result<Self, String> {
        let value: Self = ron::from_str(source).map_err(|e| e.to_string())?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<(), String> {
        self.archive.validate()?;
        self.physics.validate()?;
        self.genome.validate()?;
        self.combine.validate()?;
        self.world.validate()?;
        self.ecology.validate()?;
        self.reproduction.validate()?;
        Ok(())
    }
    pub fn bundled() -> Self {
        Self::parse(include_str!("../../../config/world.ron"))
            .expect("bundled config must be valid")
    }
}
impl PhysicsConfig {
    pub fn validate(&self) -> Result<(), String> {
        let positive = [
            self.gravity,
            self.force_per_area,
            self.length_tension_width,
            self.fixture_frequency,
            self.fixture_muscle_area,
            self.benchmark_trial_seconds,
        ];
        let nonnegative = [
            self.friction,
            self.passive_stiffness,
            self.damping,
            self.isometric_cost,
            self.audit_absolute_tolerance,
            self.audit_relative_tolerance,
        ];
        if positive.iter().any(|v| !v.is_finite() || *v <= 0.0)
            || nonnegative.iter().any(|v| !v.is_finite() || *v < 0.0)
            || !self.max_contraction_fraction.is_finite()
            || !(0.0..1.0).contains(&self.max_contraction_fraction)
            || !(1..=16).contains(&self.substeps)
        {
            return Err("invalid physics configuration range".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CombineConfig {
    pub trial_seconds: f32,
    pub stand_seconds: f32,
    pub root_clearance: f32,
    pub rough_height: f32,
    pub slope_angles: Vec<f32>,
    pub water_density: f32,
    pub water_drag: f32,
    pub carry_fraction: f32,
}
impl CombineConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !self.trial_seconds.is_finite()
            || self.trial_seconds < 2.0
            || self.trial_seconds > 60.0
            || !self.stand_seconds.is_finite()
            || self.stand_seconds < 2.0
            || self.stand_seconds > self.trial_seconds
            || [
                self.root_clearance,
                self.rough_height,
                self.water_density,
                self.water_drag,
                self.carry_fraction,
            ]
            .iter()
            .any(|x| !x.is_finite() || *x < 0.0)
            || self.slope_angles.is_empty()
            || self.slope_angles.len() > 16
            || self
                .slope_angles
                .iter()
                .any(|x| !x.is_finite() || !(0.0..1.2).contains(x))
        {
            return Err("invalid combine configuration".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_schema_rejects_unknown_missing_and_invalid_fields() {
        assert!(Config::parse("(physics: ())").is_err());
        let source = include_str!("../../../config/world.ron");
        for (from, to) in [
            ("gravity: 9.81", "gravity: -1.0"),
            ("substeps: 2", "substeps: 0"),
            ("friction: 0.9", "unknown: 0.9"),
            (
                "max_contraction_fraction: 0.55",
                "max_contraction_fraction: 1.0",
            ),
        ] {
            assert!(Config::parse(&source.replace(from, to)).is_err());
        }
        let mut c = Config::bundled().physics;
        c.force_per_area = f32::NAN;
        assert!(c.validate().is_err());
    }
}
