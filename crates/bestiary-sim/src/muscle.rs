use crate::config::PhysicsConfig;

#[derive(Clone, Copy, Debug)]
pub struct MuscleForce {
    pub active: f32,
    pub passive: f32,
    pub spring_energy: f32,
    pub isometric_power: f32,
}

/// Positive axial velocity is extension. Positive force always pulls endpoints together.
pub fn force(
    config: &PhysicsConfig,
    rest: f32,
    length: f32,
    velocity: f32,
    area: f32,
    activation: f32,
) -> MuscleForce {
    let activation = activation.clamp(0.0, 1.0);
    let stretch = (length - rest).max(0.0);
    let relative = (length / rest - 1.0) / config.length_tension_width;
    let tension = (1.0 - relative * relative).max(0.0);
    let active = if length >= rest * (1.0 - config.max_contraction_fraction) {
        config.force_per_area * area * activation * tension
    } else {
        0.0
    };
    MuscleForce {
        active,
        // A slack cable cannot push; damping acts only while stretched.
        passive: if stretch > 0.0 {
            (config.passive_stiffness * stretch + config.damping * velocity).max(0.0)
        } else {
            0.0
        },
        spring_energy: 0.5 * config.passive_stiffness * stretch * stretch,
        isometric_power: config.isometric_cost * area * activation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    #[test]
    fn muscle_is_contraction_only_and_respects_length_tension_and_area() {
        let c = Config::bundled().physics;
        let at_rest = force(&c, 1.0, 1.0, 0.0, 1.0, 1.0);
        assert_eq!(at_rest.active, c.force_per_area);
        assert_eq!(
            force(&c, 1.0, 1.0, 0.0, 2.0, 1.0).active,
            at_rest.active * 2.0
        );
        assert_eq!(force(&c, 1.0, 0.1, 0.0, 1.0, 1.0).active, 0.0);
        assert_eq!(force(&c, 1.0, 4.0, 0.0, 1.0, 1.0).active, 0.0);
        assert_eq!(force(&c, 1.0, 1.0, 0.0, 1.0, -1.0).active, 0.0);
        for length in [0.1, 0.5, 1.0, 1.2, 2.0] {
            for velocity in [-100.0, 0.0, 100.0] {
                let f = force(&c, 1.0, length, velocity, 1.0, 1.0);
                assert!(f.active >= 0.0 && f.passive >= 0.0);
            }
        }
    }
    #[test]
    fn passive_spring_energy_and_isometric_cost_are_accounted() {
        let c = Config::bundled().physics;
        let f = force(&c, 1.0, 1.5, 0.0, 2.0, 0.5);
        assert_eq!(f.spring_energy, 0.5 * c.passive_stiffness * 0.25);
        assert_eq!(f.isometric_power, c.isometric_cost);
        assert_eq!(force(&c, 1.0, 0.5, 0.0, 2.0, 0.0).spring_energy, 0.0);
    }
}
