//! Species registration and persistent records, independent of world implementation.
pub mod grid;
pub mod store;
use bestiary_core::{genome::Genome, settings::ReproductionConfig};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Species {
    pub id: u64,
    pub name: String,
    pub founder: [u8; 32],
    pub born: u64,
    pub extinct: Option<u64>,
    pub parent: Option<u64>,
    pub population: usize,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Registry {
    pub species: BTreeMap<u64, Species>,
    pub assignments: BTreeMap<[u8; 32], u64>,
    pub pending: BTreeMap<[u8; 32], u64>,
    pub next_id: u64,
}
pub struct Population<'a> {
    pub hash: [u8; 32],
    pub genome: &'a Genome,
    pub traits: Vec<f32>,
    pub count: usize,
    pub parents: &'a [[u8; 32]],
}
impl Registry {
    pub fn update(&mut self, population: &[Population<'_>], tick: u64, cfg: &ReproductionConfig) {
        let old = self.assignments.clone();
        let mut groups: Vec<(usize, Vec<usize>)> = vec![];
        for (i, p) in population.iter().enumerate() {
            let group = groups.iter().position(|(rep, _)| {
                p.genome
                    .compatibility(population[*rep].genome, &p.traits, &population[*rep].traits)
                    < cfg.compatibility_threshold
            });
            if let Some(group) = group {
                groups[group].1.push(i);
            } else {
                groups.push((i, vec![i]));
            }
        }
        for s in self.species.values_mut() {
            s.population = 0;
        }
        // Keep historical genome assignments so descendant parent links survive extinction.
        let mut pending = BTreeMap::new();
        let mut used_ids = std::collections::BTreeSet::new();
        for (representative, members) in groups {
            let p = &population[representative];
            let count = members.iter().map(|i| population[*i].count).sum::<usize>();
            let existing = members
                .iter()
                .filter_map(|i| old.get(&population[*i].hash))
                .copied()
                .filter(|id| !used_ids.contains(id))
                .min();
            let id = if let Some(id) = existing {
                Some(id)
            } else {
                let first = *self.pending.get(&p.hash).unwrap_or(&tick);
                if count >= cfg.species_min_population {
                    pending.insert(p.hash, first);
                }
                if count >= cfg.species_min_population && tick - first >= cfg.species_min_duration {
                    self.next_id += 1;
                    let id = self.next_id;
                    let parent = p.parents.iter().filter_map(|h| old.get(h)).copied().min();
                    let genera = [
                        "Aster", "Bruma", "Cressa", "Droma", "Ephyra", "Folia", "Galea", "Helia",
                    ];
                    self.species.insert(
                        id,
                        Species {
                            id,
                            name: format!("{} forma{}", genera[id as usize % genera.len()], id),
                            founder: p.hash,
                            born: tick,
                            extinct: None,
                            parent,
                            population: 0,
                        },
                    );
                    Some(id)
                } else {
                    None
                }
            };
            if let Some(id) = id {
                used_ids.insert(id);
                let species = self.species.get_mut(&id).expect("registered species");
                species.population += count;
                species.extinct = None;
                for i in members {
                    self.assignments.insert(population[i].hash, id);
                }
            }
        }
        self.pending = pending;
        for s in self.species.values_mut() {
            if s.population == 0 && s.extinct.is_none() {
                s.extinct = Some(tick);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn species_minimum_duration_parent_split_and_extinction() {
        let a = Genome::starter(&bestiary_sim::config::Config::bundled().genome);
        let mut b = a.clone();
        b.life.species_marker = [1.0; 4];
        let cfg = ReproductionConfig {
            epoch: 1,
            mate_radius: 10.0,
            compatibility_threshold: 0.001,
            energy_fraction: 0.5,
            gestation_loss: 0.1,
            mutation_probability: 0.1,
            founder_maturity: 1,
            founder_senescence: 100,
            species_epoch: 1,
            species_min_population: 2,
            species_min_duration: 2,
        };
        let mut registry = Registry::default();
        let p = [Population {
            hash: a.hash(),
            genome: &a,
            traits: vec![1.0],
            count: 2,
            parents: &[],
        }];
        registry.update(&p, 0, &cfg);
        assert!(registry.species.is_empty());
        registry.update(&p, 2, &cfg);
        assert_eq!(registry.species.len(), 1);
        let parents = [a.hash()];
        let p = [Population {
            hash: b.hash(),
            genome: &b,
            traits: vec![2.0],
            count: 2,
            parents: &parents,
        }];
        registry.update(&p, 3, &cfg);
        registry.update(&p, 5, &cfg);
        assert_eq!(registry.species[&2].parent, Some(1));
        assert_eq!(registry.species[&1].extinct, Some(3));
        registry.update(&[], 6, &cfg);
        assert_eq!(registry.species[&2].extinct, Some(6));
    }
}
