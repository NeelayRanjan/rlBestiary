//! Sparse MAP-Elites with a fixed, conservative coverage denominator.
use bestiary_core::settings::ArchiveConfig;
use bestiary_sim::combine::{Evaluation, Traits};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub type CellKey = [u8; 7];
pub const AXES: [&str; 7] = [
    "Parts",
    "Limbs",
    "Mass",
    "Height",
    "Symmetry",
    "Gait frequency",
    "Aquatic speed",
];
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Elite {
    pub genome: [u8; 32],
    pub traits: Traits,
    pub viability: f32,
    pub discovered: u64,
    pub replay_id: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Archive {
    pub cells: BTreeMap<CellKey, Elite>,
}
pub fn descriptors(t: &Traits) -> [f32; 7] {
    [
        t.parts as f32,
        t.limbs as f32,
        t.mass,
        t.reach,
        t.symmetry,
        t.gait_frequency,
        t.swim_speed,
    ]
}
pub fn cell(t: &Traits, config: &ArchiveConfig) -> CellKey {
    let values = descriptors(t);
    std::array::from_fn(|i| config.thresholds[i].partition_point(|cut| values[i] >= *cut) as u8)
}
impl Archive {
    /// Return Some(key) only when a previously empty cell is filled.
    pub fn insert(
        &mut self,
        evaluation: &Evaluation,
        tick: u64,
        config: &ArchiveConfig,
    ) -> Option<CellKey> {
        if evaluation.exploit_suspect
            || evaluation.viability <= 0.0
            || !evaluation.viability.is_finite()
            || descriptors(&evaluation.traits)
                .iter()
                .any(|x| !x.is_finite())
        {
            return None;
        }
        let key = cell(&evaluation.traits, config);
        let empty = !self.cells.contains_key(&key);
        if self.cells.get(&key).is_none_or(|old| {
            evaluation.viability > old.viability
                || (evaluation.viability == old.viability && evaluation.genome_hash < old.genome)
        }) {
            let hash = bestiary_core::hex(&evaluation.genome_hash);
            self.cells.insert(
                key,
                Elite {
                    genome: evaluation.genome_hash,
                    traits: evaluation.traits.clone(),
                    viability: evaluation.viability,
                    discovered: tick,
                    replay_id: format!("{hash}-0"),
                },
            );
        }
        empty.then_some(key)
    }
    pub fn coverage(&self, config: &ArchiveConfig) -> f32 {
        self.cells.len() as f32 / config.total_cells() as f32
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use bestiary_core::genome::Genome;
    use bestiary_sim::{combine::evaluate, config::Config};
    #[test]
    fn sparse_elites_exclude_exploits_and_recombine_exactly() {
        let c = Config::bundled();
        let g = Genome::starter(&c.genome);
        let e = evaluate(&g, &c).unwrap();
        let mut archive = Archive::default();
        assert!(archive.insert(&e, 0, &c.archive).is_some());
        let recomputed = evaluate(&g, &c).unwrap();
        assert_eq!(
            archive.cells.values().next().unwrap().traits,
            recomputed.traits
        );
        assert!(archive.insert(&recomputed, 1, &c.archive).is_none());
        let mut bad = e.clone();
        bad.exploit_suspect = true;
        bad.genome_hash = [0; 32];
        bad.viability = 1.0;
        archive.insert(&bad, 2, &c.archive);
        assert_eq!(archive.cells.values().next().unwrap().genome, e.genome_hash);
        let mut tied = e.clone();
        tied.genome_hash = [0; 32];
        archive.insert(&tied, 3, &c.archive);
        assert_eq!(archive.cells.values().next().unwrap().genome, [0; 32]);
        assert_eq!(archive.coverage(&c.archive), 1.0 / 128.0);
    }
    #[test]
    fn archive_schema_and_boundaries() {
        let mut c = Config::bundled().archive;
        assert_eq!(c.total_cells(), 128);
        c.thresholds[0] = vec![2.0, 1.0];
        assert!(c.validate().is_err());
        c.thresholds[0] = vec![f32::NAN];
        assert!(c.validate().is_err());
        c.thresholds[0] = vec![];
        assert!(c.validate().is_err());
    }
}
