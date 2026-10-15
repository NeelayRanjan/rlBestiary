use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
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
        value.physics.validate()?;
        Ok(value)
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_schema_rejects_unknown_missing_and_invalid_fields() {
        assert!(Config::parse("(physics: ())").is_err());
        let source = include_str!("../../../config/world.ron");
        for (from, to) in [
            ("gravity: 9.81", "gravity: -1.0"),
            ("substeps: 1", "substeps: 0"),
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
