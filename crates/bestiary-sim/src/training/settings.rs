use serde::{Deserialize, Serialize};

/// Training knobs, loaded from `config/training.ron`. Not world parameters:
/// they never reach world files, trait caches or state hashes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainingSettings {
    pub population: usize,
    pub elite_fraction: f32,
    pub generations: u32,
    pub slope_angle: f32,
    pub weight_rate: f32,
    pub weight_sigma: f32,
    pub oscillator_sigma: f32,
    pub activation_rate: f32,
    pub body_mutation: bool,
}
impl TrainingSettings {
    pub fn parse(source: &str) -> Result<Self, String> {
        let s: Self = ron::from_str(source).map_err(|e| e.to_string())?;
        s.validate()?;
        Ok(s)
    }
    pub fn bundled() -> Self {
        Self::parse(include_str!("../../../../config/training.ron"))
            .expect("bundled training settings must be valid")
    }
    pub fn validate(&self) -> Result<(), String> {
        let unit = |x: f32| x.is_finite() && (0.0..=1.0).contains(&x);
        if !(2..=512).contains(&self.population) {
            return Err("population must be 2..=512".into());
        }
        if !(self.elite_fraction.is_finite()
            && self.elite_fraction > 0.0
            && self.elite_fraction < 1.0)
        {
            return Err("elite_fraction must be strictly between 0 and 1".into());
        }
        if self.generations == 0 {
            return Err("generations must be positive".into());
        }
        if !(self.slope_angle.is_finite() && (0.0..1.5).contains(&self.slope_angle)) {
            return Err("slope_angle must be in [0, 1.5) radians".into());
        }
        if !unit(self.weight_rate) || !unit(self.activation_rate) {
            return Err("rates must be in [0, 1]".into());
        }
        for (name, v) in [
            ("weight_sigma", self.weight_sigma),
            ("oscillator_sigma", self.oscillator_sigma),
        ] {
            if !(v.is_finite() && (0.0..=16.0).contains(&v)) {
                return Err(format!("{name} must be in [0, 16]"));
            }
        }
        Ok(())
    }
    /// Elites kept per generation: at least one, and at least one child slot.
    pub fn elites(&self) -> usize {
        ((self.population as f32 * self.elite_fraction).ceil() as usize)
            .clamp(1, self.population - 1)
    }
    pub fn controller(&self) -> bestiary_core::genome::ControllerMutation {
        bestiary_core::genome::ControllerMutation {
            weight_rate: self.weight_rate,
            weight_sigma: self.weight_sigma,
            oscillator_sigma: self.oscillator_sigma,
            activation_rate: self.activation_rate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_training_settings_are_valid() {
        let s = TrainingSettings::bundled();
        assert_eq!(s.elites(), 10);
        assert!(!s.body_mutation);
    }
    #[test]
    fn training_settings_reject_degenerate_values() {
        let base = TrainingSettings::bundled();
        let cases: Vec<fn(&mut TrainingSettings)> = vec![
            |s| s.population = 0,
            |s| s.population = 1,
            |s| s.population = 513,
            |s| s.elite_fraction = 0.0,
            |s| s.elite_fraction = 1.0,
            |s| s.elite_fraction = f32::NAN,
            |s| s.weight_rate = -0.1,
            |s| s.weight_rate = 1.5,
            |s| s.weight_sigma = f32::INFINITY,
            |s| s.oscillator_sigma = -1.0,
            |s| s.activation_rate = 2.0,
            |s| s.slope_angle = 1.6,
            |s| s.generations = 0,
        ];
        for (i, case) in cases.into_iter().enumerate() {
            let mut s = base.clone();
            case(&mut s);
            assert!(s.validate().is_err(), "case {i} accepted");
        }
    }
    #[test]
    fn training_settings_reject_unknown_fields() {
        let source =
            include_str!("../../../../config/training.ron").replace("population:", "populaton:");
        assert!(TrainingSettings::parse(&source).is_err());
    }
    #[test]
    fn small_population_keeps_at_least_one_elite_and_one_child() {
        let mut s = TrainingSettings::bundled();
        s.population = 2;
        s.elite_fraction = 0.9;
        assert_eq!(s.elites(), 1);
    }
}
