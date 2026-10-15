//! Repeatable mechanical gait measurements; no controller or pose correction.
use crate::{
    combine::{Replay, TrialKind},
    config::Config,
    genome::Creature,
    PHYSICS_DT,
};
use bestiary_core::genome::Genome;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FootReport {
    pub part: usize,
    pub contact_fraction: f32,
    pub maximum_swing_clearance: f32,
    /// Landings preceded by at least two centimetres of measured clearance.
    pub completed_swings: usize,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GaitReport {
    pub measured_seconds: f32,
    pub net_translation: [f32; 3],
    pub root_min_clearance: f32,
    pub root_contact_fraction: f32,
    pub support_fraction: f32,
    pub tripod_support_fraction: f32,
    pub mean_supporting_feet: f32,
    pub longest_unsupported_seconds: f32,
    pub root_vertical_range: f32,
    pub minimum_upright_cosine: f32,
    /// Normal-load weighted speed at actual terrain contact points, not foot COM.
    pub stance_slip_speed: f32,
    pub active_work: f64,
    pub isometric_cost: f64,
    pub exploit_suspect: bool,
    pub feet: Vec<FootReport>,
}
/// Measure after an unscored settling interval. The optional ablation removes
/// active muscle force while retaining the same passive material and physics.
/// The replay stores every fixed step, including settling, for exact playback.
pub fn measure(
    genome: &Genome,
    config: &Config,
    settle_seconds: f32,
    seconds: f32,
    driven: bool,
) -> Result<(GaitReport, Replay), String> {
    if !settle_seconds.is_finite()
        || settle_seconds < 0.0
        || !seconds.is_finite()
        || seconds <= 0.0
        || settle_seconds + seconds > 120.0
    {
        return Err("gait measurement must be finite, positive and at most 120 seconds".into());
    }
    let warmup = (settle_seconds / PHYSICS_DT).round() as usize;
    let steps = (seconds / PHYSICS_DT).round() as usize;
    if steps == 0 {
        return Err("gait measurement needs at least one fixed step".into());
    }
    let mut c = Creature::new(genome.clone(), config)?;
    let mut feet: Vec<_> = c
        .body
        .parts
        .iter()
        .enumerate()
        .filter(|(i, p)| *i != 0 && p.children.iter().all(Option::is_none))
        .map(|(part, _)| FootReport {
            part,
            contact_fraction: 0.0,
            maximum_swing_clearance: 0.0,
            completed_swings: 0,
        })
        .collect();
    let mut cleared = vec![false; feet.len()];
    let mut frames = Vec::with_capacity(warmup + steps + 1);
    frames.push(c.arena.poses());
    let root = c.arena.parts[0];
    let mut start = *c.arena.bodies[root].translation();
    let mut report = GaitReport {
        measured_seconds: steps as f32 * PHYSICS_DT,
        net_translation: [0.0; 3],
        root_min_clearance: f32::INFINITY,
        root_contact_fraction: 0.0,
        support_fraction: 0.0,
        tripod_support_fraction: 0.0,
        mean_supporting_feet: 0.0,
        longest_unsupported_seconds: 0.0,
        root_vertical_range: 0.0,
        minimum_upright_cosine: 1.0,
        stance_slip_speed: 0.0,
        active_work: 0.0,
        isometric_cost: 0.0,
        exploit_suspect: false,
        feet: vec![],
    };
    let mut low = f32::INFINITY;
    let mut high = f32::NEG_INFINITY;
    let mut load = 0.0;
    let mut unsupported = 0;
    let mut start_work = 0.0;
    let mut start_hold = 0.0;
    let inactive = vec![0.0; c.arena.muscles.len()];
    for tick in 0..warmup + steps {
        if tick == warmup {
            start = *c.arena.bodies[root].translation();
            start_work = c.arena.audit.active_work;
            start_hold = c.arena.audit.isometric_cost;
        }
        if driven {
            c.step();
        } else {
            c.arena.step(&inactive);
        }
        frames.push(c.arena.poses());
        if tick < warmup {
            continue;
        }
        let rigid = &c.arena.bodies[root];
        let bottom = rigid
            .colliders()
            .iter()
            .map(|h| c.arena.colliders[*h].compute_aabb().mins.y)
            .fold(f32::INFINITY, f32::min);
        report.root_min_clearance = report.root_min_clearance.min(bottom);
        report.root_contact_fraction += f32::from(c.arena.touches_terrain(root));
        low = low.min(rigid.translation().y);
        high = high.max(rigid.translation().y);
        report.minimum_upright_cosine = report
            .minimum_upright_cosine
            .min((rigid.rotation() * nalgebra::Vector3::y()).y);
        let mut support = 0;
        for (i, foot) in feet.iter_mut().enumerate() {
            let handle = c.arena.parts[foot.part];
            let contacts = c.arena.terrain_contacts(handle);
            if contacts.is_empty() {
                let bottom = c.arena.bodies[handle]
                    .colliders()
                    .iter()
                    .map(|h| c.arena.colliders[*h].compute_aabb().mins.y)
                    .fold(f32::INFINITY, f32::min);
                foot.maximum_swing_clearance = foot.maximum_swing_clearance.max(bottom);
                cleared[i] |= bottom > 0.02;
            } else {
                support += 1;
                foot.contact_fraction += 1.0;
                if cleared[i] {
                    foot.completed_swings += 1;
                    cleared[i] = false;
                }
                for contact in contacts {
                    report.stance_slip_speed += contact.normal_force * contact.tangential_speed;
                    load += contact.normal_force;
                }
            }
        }
        report.mean_supporting_feet += support as f32;
        report.tripod_support_fraction += f32::from(support >= 3);
        report.support_fraction += f32::from(support > 0);
        unsupported = if support == 0 { unsupported + 1 } else { 0 };
        report.longest_unsupported_seconds = report
            .longest_unsupported_seconds
            .max(unsupported as f32 * PHYSICS_DT);
    }
    report.net_translation = (*c.arena.bodies[root].translation() - start).into();
    report.root_vertical_range = high - low;
    report.root_contact_fraction /= steps as f32;
    report.support_fraction /= steps as f32;
    report.tripod_support_fraction /= steps as f32;
    report.mean_supporting_feet /= steps as f32;
    report.stance_slip_speed = if load > 0.0 {
        report.stance_slip_speed / load
    } else {
        0.0
    };
    for f in &mut feet {
        f.contact_fraction /= steps as f32;
    }
    report.feet = feet;
    report.active_work = c.arena.audit.active_work - start_work;
    report.isometric_cost = c.arena.audit.isometric_cost - start_hold;
    report.exploit_suspect = c.arena.audit.exploit_suspect;
    Ok((
        report,
        Replay {
            kind: TrialKind::Sprint,
            dt: PHYSICS_DT,
            frames,
        },
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coordinated_walking_has_proper_foot_contact_and_is_muscle_driven() {
        let c = Config::bundled();
        let g = Genome::starter(&c.genome);
        let (a, replay) = measure(&g, &c, 2.0, 8.0, true).unwrap();
        let (repeat, again) = measure(&g, &c, 2.0, 8.0, true).unwrap();
        assert_eq!(a, repeat);
        assert_eq!(replay, again);
        let (passive, _) = measure(&g, &c, 2.0, 8.0, false).unwrap();
        let (old, _) = measure(&Genome::rigid_limb_fixture(&c.genome), &c, 2.0, 8.0, true).unwrap();
        println!("walking={a:?}\npassive={passive:?}\nrigid-limb baseline={old:?}");
        assert!(!a.exploit_suspect);
        assert_eq!(a.root_contact_fraction, 0.0);
        assert!(a.root_min_clearance > 0.35);
        assert!(a.net_translation[0] > 1.5);
        assert!(a.net_translation[2].abs() < a.net_translation[0] * 0.25);
        assert!(a.minimum_upright_cosine > 0.97);
        assert!(a.root_vertical_range < 0.06);
        assert!(a.support_fraction > 0.98);
        assert!(a.longest_unsupported_seconds < 0.05);
        assert!(a.stance_slip_speed < 0.10 && a.stance_slip_speed < old.stance_slip_speed * 0.5);
        assert_eq!(a.feet.len(), 6);
        assert!(a.feet.iter().all(|f| f.maximum_swing_clearance > 0.03
            && f.completed_swings >= 4
            && f.contact_fraction > 0.15));
        assert!(a.active_work > 0.0 && a.isometric_cost > 0.0);
        assert_eq!(passive.active_work, 0.0);
        assert!(passive.net_translation[0].abs() < a.net_translation[0] * 0.15);
    }
    #[test]
    fn gait_measurement_rejects_invalid_durations() {
        let c = Config::bundled();
        let g = Genome::starter(&c.genome);
        for (settle, seconds) in [(-1.0, 8.0), (2.0, 0.0), (2.0, f32::NAN), (2.0, 121.0)] {
            assert!(measure(&g, &c, settle, seconds, true).is_err());
        }
    }
}
