use super::*;
use bestiary_archive::{
    store::{CreatureRow, GenomeRow, MutationRow, Persistable, Records, ReplayRow, TimeSample},
    Population,
};
use bestiary_core::genome::Operator;
use std::collections::BTreeSet;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LifeRecord {
    pub id: u64,
    pub genome: [u8; 32],
    pub born: u64,
    pub died: Option<u64>,
    pub species: u64,
    pub cause: Option<String>,
    pub offspring: u64,
    pub parents: Vec<u64>,
}
impl World {
    pub fn register_genome(
        &mut self,
        genome: Genome,
        parents: Vec<[u8; 32]>,
    ) -> Result<[u8; 32], String> {
        let hash = genome.hash();
        if self.genomes.contains_key(&hash) {
            return Ok(hash);
        }
        let evaluation = evaluate(&genome, &self.config)?;
        if let Some(cell) = self
            .archive
            .insert(&evaluation, self.tick, &self.config.archive)
        {
            self.ecological_events
                .push(EcologyEvent::ArchiveCell { cell, genome: hash });
        }
        let replays = evaluation
            .trials
            .iter()
            .filter_map(|t| t.replay.as_ref())
            .map(|r| r.encode())
            .collect::<Result<Vec<_>, _>>()?;
        if evaluation.exploit_suspect {
            self.ecological_events
                .push(EcologyEvent::Exploit { genome: hash });
        }
        self.record_body_milestones(&genome, &evaluation);
        if self
            .genomes
            .values()
            .all(|g| evaluation.traits.mass > g.traits.mass)
        {
            self.ecological_events.push(EcologyEvent::RecordSize {
                genome: hash,
                mass: evaluation.traits.mass,
            });
        }
        self.genomes.insert(
            hash,
            LineageGenome {
                genome,
                traits: evaluation.traits,
                viability: evaluation.viability,
                exploit: evaluation.exploit_suspect,
                discovered: self.tick,
                parents,
                replays,
            },
        );
        Ok(hash)
    }
    fn mating_pairs(
        &self,
        cfg: &bestiary_core::settings::ReproductionConfig,
    ) -> Vec<(usize, usize)> {
        if self.agents.len() >= self.config.world.population_limit {
            return vec![];
        }
        let regions = self.land_regions();
        let extent = self.config.world.grid as f32 * self.config.world.cell_metres;
        let mut eligible = self
            .agents
            .iter()
            .enumerate()
            .filter_map(|(index, agent)| {
                let genome = &self.genomes[&agent.genome];
                (agent.health > 0.0
                    && agent.age >= genome.genome.life.maturity
                    && agent.growth >= 0.8
                    && agent.energy as f32
                        >= genome.traits.mass
                            * self.config.world.reserve_per_kg as f32
                            * cfg.energy_fraction
                    && agent.outputs.get(7).copied().unwrap_or(0.0)
                        > agent.outputs.get(8).copied().unwrap_or(0.0))
                .then_some(index)
            })
            .collect::<Vec<_>>();
        eligible.sort_by_key(|index| self.agents[*index].id);
        let preference = |genome: &Genome, traits: &Traits, other: &Genome| {
            genome
                .life
                .mate_weights
                .iter()
                .zip([
                    traits.mass,
                    traits.reach,
                    traits.symmetry,
                    other.life.pheromone_strength,
                ])
                .map(|(weight, value)| weight * value)
                .sum::<f32>()
        };
        let mut used = BTreeSet::new();
        let mut pairs = vec![];
        for &i in &eligible {
            if used.contains(&i) {
                continue;
            }
            let a = &self.agents[i];
            let ga = &self.genomes[&a.genome];
            let mut best: Option<(f32, u64, usize)> = None;
            for &j in &eligible {
                if j == i || used.contains(&j) {
                    continue;
                }
                let b = &self.agents[j];
                let mut delta = [b.position[0] - a.position[0], b.position[1] - a.position[1]];
                for coordinate in &mut delta {
                    if *coordinate > extent * 0.5 {
                        *coordinate -= extent;
                    }
                    if *coordinate < -extent * 0.5 {
                        *coordinate += extent;
                    }
                }
                let distance = delta[0] * delta[0] + delta[1] * delta[1];
                if distance > cfg.mate_radius * cfg.mate_radius
                    || best.is_some_and(|(d, id, _)| distance > d || (distance == d && b.id > id))
                {
                    continue;
                }
                let gb = &self.genomes[&b.genome];
                if ga.traits.speed > ga.traits.swim_speed && gb.traits.speed > gb.traits.swim_speed
                {
                    let ra = regions[self.cell_at(a.position)];
                    let rb = regions[self.cell_at(b.position)];
                    if ra == 0 || ra != rb {
                        continue;
                    }
                }
                if a.genome != b.genome
                    && ga
                        .genome
                        .compatibility(&gb.genome, &ga.traits.values(), &gb.traits.values())
                        >= cfg.compatibility_threshold
                {
                    continue;
                }
                if preference(&ga.genome, &gb.traits, &gb.genome) < -1.0
                    || preference(&gb.genome, &ga.traits, &ga.genome) < -1.0
                {
                    continue;
                }
                best = Some((distance, b.id, j));
            }
            if let Some((_, _, j)) = best {
                used.insert(i);
                used.insert(j);
                pairs.push((i, j));
            }
        }
        pairs
    }

    pub(crate) fn reproduce(&mut self) {
        let cfg = self.config.reproduction.clone();
        if self.tick % cfg.epoch == 0 {
            let proposals = self.mating_pairs(&cfg);
            // All evaluation happens synchronously in proposal order. Worker wall-clock cannot change birth order.
            for (i, j) in proposals {
                if self.agents.len() >= self.config.world.population_limit {
                    break;
                }
                let pa = self.agents[i].genome;
                let pb = self.agents[j].genome;
                let mut child = self.genomes[&pa].genome.crossover(
                    &self.genomes[&pb].genome,
                    self.seed ^ self.tick ^ self.agents[i].id,
                    &self.config.genome,
                );
                let mut parent_hash = match self.register_genome(child.clone(), vec![pa, pb]) {
                    Ok(h) => h,
                    Err(_) => continue,
                };
                let mut rng =
                    bestiary_core::rng(self.seed, 0x4d555442, self.agents[i].id, self.tick);
                let start = self.mutations.len();
                if rng.gen::<f32>() < cfg.mutation_probability {
                    for (oi, op) in Operator::ALL.iter().enumerate() {
                        if rng.gen::<f32>() >= child.mutation_rates[oi] {
                            continue;
                        }
                        let (next, mut record) = child.mutate(
                            *op,
                            self.seed ^ self.agents[i].id,
                            self.tick,
                            &self.config.genome,
                        );
                        let hash = match self.register_genome(next.clone(), vec![parent_hash]) {
                            Ok(h) => h,
                            Err(_) => continue,
                        };
                        let result = &self.genomes[&hash];
                        record.traits = Some(result.traits.values());
                        record.viability = Some(result.viability);
                        self.mutations.push((self.tick, record));
                        child = next;
                        parent_hash = hash;
                    }
                }
                if self.genomes[&parent_hash].exploit || self.genomes[&parent_hash].viability <= 0.0
                {
                    continue;
                }
                let fraction = child.life.offspring_fraction.max(0.01);
                let count = usize::from(child.life.clutch_size)
                    .min(self.config.world.population_limit - self.agents.len());
                let mass = self.genomes[&parent_hash].traits.mass;
                let nutrient_per_child =
                    (mass * fraction * self.config.world.nutrient_units_per_kg) as i64;
                let energy_per_child =
                    (mass * fraction * self.config.world.reserve_per_kg as f32 * 0.5) as i64;
                let needed_nutrients = nutrient_per_child * count as i64;
                let transferred_energy = energy_per_child * count as i64;
                let gestation = (transferred_energy as f32 * cfg.gestation_loss) as i64;
                let needed_energy = transferred_energy + gestation;
                if self.agents[i].nutrients < needed_nutrients / 2
                    || self.agents[j].nutrients < needed_nutrients - needed_nutrients / 2
                    || self.agents[i].energy < needed_energy / 2
                    || self.agents[j].energy < needed_energy - needed_energy / 2
                {
                    continue;
                }
                self.agents[i].energy -= needed_energy / 2;
                self.agents[j].energy -= needed_energy - needed_energy / 2;
                self.dissipated += gestation;
                self.agents[i].nutrients -= needed_nutrients / 2;
                self.agents[j].nutrients -= needed_nutrients - needed_nutrients / 2;
                for index in [i, j] {
                    let m = self.genomes[&self.agents[index].genome].traits.mass;
                    self.agents[index].growth = (self.agents[index].nutrients as f32
                        / (m * self.config.world.nutrient_units_per_kg))
                        .min(1.0);
                    self.agents[index].offspring += count as u64;
                }
                let parents = vec![self.agents[i].id, self.agents[j].id];
                let position = self.agents[i].position;
                for _ in 0..count {
                    let id = self.next_id;
                    self.next_id += 1;
                    let a = Agent {
                        pheromone_guidance: None,
                        infection: None,
                        id,
                        genome: parent_hash,
                        position,
                        energy: energy_per_child,
                        nutrients: nutrient_per_child,
                        age: 0,
                        health: 1.0,
                        growth: fraction,
                        injury: 0.0,
                        brain: child.ethology.state(),
                        outputs: vec![0.0; child.ethology.neurons.len()],
                        species: 0,
                        parents: parents.clone(),
                        offspring: 0,
                    };
                    self.history.insert(
                        id,
                        LifeRecord {
                            id,
                            genome: parent_hash,
                            born: self.tick,
                            died: None,
                            species: 0,
                            cause: None,
                            offspring: 0,
                            parents: parents.clone(),
                        },
                    );
                    self.ecological_events.push(EcologyEvent::Birth {
                        creature: id,
                        parents: parents.clone(),
                    });
                    self.agents.push(a);
                }
                for (_, record) in &mut self.mutations[start..] {
                    if record.child == parent_hash {
                        record.born = true;
                    }
                }
            }
        }
        if self.tick % cfg.species_epoch == 0 {
            let mut counts = BTreeMap::new();
            for a in &self.agents {
                if a.health > 0.0 {
                    *counts.entry(a.genome).or_insert(0usize) += 1;
                }
            }
            let ancestry: BTreeMap<_, Vec<_>> = counts
                .keys()
                .map(|hash| {
                    let mut seen = BTreeSet::new();
                    let mut stack = self.genomes[hash].parents.clone();
                    while let Some(parent) = stack.pop() {
                        if seen.insert(parent) {
                            if let Some(g) = self.genomes.get(&parent) {
                                stack.extend(&g.parents);
                            }
                        }
                    }
                    (*hash, seen.into_iter().collect())
                })
                .collect();
            let population: Vec<_> = counts
                .into_iter()
                .map(|(hash, count)| {
                    let entry = &self.genomes[&hash];
                    Population {
                        hash,
                        genome: &entry.genome,
                        traits: entry.traits.values(),
                        count,
                        parents: &ancestry[&hash],
                    }
                })
                .collect();
            let previous_id = self.registry.next_id;
            let old_living: BTreeSet<_> = self
                .registry
                .species
                .values()
                .filter(|s| s.population > 0)
                .map(|s| s.id)
                .collect();
            self.registry.update(&population, self.tick, &cfg);
            for s in self.registry.species.values() {
                if s.id > previous_id {
                    self.ecological_events
                        .push(EcologyEvent::SpeciesBorn { species: s.id });
                }
                if s.population == 0 && old_living.contains(&s.id) {
                    self.ecological_events
                        .push(EcologyEvent::SpeciesExtinct { species: s.id });
                }
            }

            for a in &mut self.agents {
                a.species = *self.registry.assignments.get(&a.genome).unwrap_or(&0);
            }
            let mut alive = BTreeSet::new();
            let mut stack: Vec<_> = self.agents.iter().map(|a| a.genome).collect();
            while let Some(hash) = stack.pop() {
                if alive.insert(hash) {
                    if let Some(g) = self.genomes.get(&hash) {
                        stack.extend(&g.parents);
                    }
                }
            }
            for (born, record) in &mut self.mutations {
                let span = self
                    .history
                    .values()
                    .filter(|h| h.genome == record.child)
                    .map(|h| h.died.unwrap_or(self.tick) - h.born)
                    .max();
                if span.is_some() {
                    record.born = true;
                    record.lifespan = span;
                }
                if alive.contains(&record.child) {
                    record.lineage_survival = Some(self.tick - *born);
                }
            }
        }
        if self.config.archive.reseed_interval > 0
            && self.tick % self.config.archive.reseed_interval == 0
        {
            let mut counts = BTreeMap::new();
            for a in &self.agents {
                *counts.entry(a.genome).or_insert(0usize) += 1;
            }
            if let Some(elite) = self
                .archive
                .cells
                .values()
                .min_by_key(|e| (counts.get(&e.genome).copied().unwrap_or(0), e.genome))
            {
                let genome = self.genomes[&elite.genome].genome.clone();
                let mut rng = bestiary_core::rng(self.seed, 0x52455345, 0, self.tick);
                let extent = self.config.world.grid as f32 * self.config.world.cell_metres;
                let position = [rng.gen_range(0.0..extent), rng.gen_range(0.0..extent)];
                let _ = self.release(genome, position, self.config.archive.reseed_count);
            }
        }
        for agent in &self.agents {
            if let Some(h) = self.history.get_mut(&agent.id) {
                h.species = agent.species;
                h.offspring = agent.offspring;
            }
        }
    }
}
impl Persistable for World {
    fn tick(&self) -> u64 {
        self.tick
    }
    fn population(&self) -> usize {
        self.agents.len()
    }
    fn biomass(&self) -> f64 {
        self.agents
            .iter()
            .map(|a| f64::from(self.genomes[&a.genome].traits.mass * a.growth))
            .sum()
    }
    fn sample(&self) -> TimeSample {
        let mut sample = TimeSample {
            tick: self.tick,
            population: self.agents.len(),
            species: self
                .registry
                .species
                .values()
                .filter(|s| s.population > 0)
                .count(),
            biomass: self.biomass(),
            species_populations: self
                .registry
                .species
                .values()
                .map(|s| (s.id, s.population))
                .collect(),
            ..TimeSample::default()
        };
        let n = self.agents.len().max(1) as f32;
        let mut herbivores = 0usize;
        for a in &self.agents {
            let g = &self.genomes[&a.genome];
            let values = g.traits.values();
            sample.mean_traits.resize(values.len(), 0.0);
            sample.trait_variance.resize(values.len(), 0.0);
            for (i, v) in values.iter().enumerate() {
                sample.mean_traits[i] += v / n;
            }
            let tier = if g.genome.life.plant_efficiency >= 0.65 {
                0
            } else if g.genome.life.plant_efficiency <= 0.35 {
                2
            } else {
                1
            };
            sample.trophic_biomass[tier] += f64::from(g.traits.mass * a.growth);
            if tier == 0 {
                sample.herbivore_resistance += g.genome.life.immune_investment;
                herbivores += 1;
            }
        }
        for a in &self.agents {
            for (i, v) in self.genomes[&a.genome].traits.values().iter().enumerate() {
                sample.trait_variance[i] += (v - sample.mean_traits[i]).powi(2) / n;
            }
        }
        sample.herbivore_resistance /= herbivores.max(1) as f32;
        sample.coverage = self.archive.coverage(&self.config.archive);
        sample.plant_toxicity = self.cells.iter().map(|c| c.plant.toxicity).sum::<f32>()
            / self.cells.len().max(1) as f32;
        sample
    }
    fn records(&self) -> Records {
        let archive = self
            .archive
            .cells
            .iter()
            .map(|(key, e)| (key.to_vec(), e.genome.to_vec(), e.viability))
            .collect();
        let mut records = Records {
            archive,
            ..Records::default()
        };
        for (hash, g) in &self.genomes {
            records.genomes.push(GenomeRow {
                hash: hash.to_vec(),
                blob: postcard::to_allocvec(&g.genome).expect("genome"),
                traits: postcard::to_allocvec(&g.traits).expect("traits"),
                viability: g.viability,
                tick: g.discovered,
                parents: postcard::to_allocvec(&g.parents).expect("parents"),
                exploit: g.exploit,
            });
            for (index, data) in g.replays.iter().enumerate() {
                records.replays.push(ReplayRow {
                    id: format!("{}-{index}", hex(hash)),
                    genome: hash.to_vec(),
                    data: data.clone(),
                });
            }
        }
        records.creatures = self
            .history
            .values()
            .map(|h| CreatureRow {
                id: h.id,
                genome: h.genome.to_vec(),
                born: h.born,
                died: h.died,
                species: h.species,
                cause: h.cause.clone(),
                offspring: h.offspring,
                parents: h.parents.clone(),
            })
            .collect();
        records.mutations = self
            .mutations
            .iter()
            .enumerate()
            .map(|(id, (tick, record))| MutationRow {
                id: id as u64,
                tick: *tick,
                record: record.clone(),
            })
            .collect();
        records.species = self.registry.species.values().cloned().collect();
        records
    }
}
pub use bestiary_core::hex;

impl World {
    /// User releases import a genome but transfer the founding population's resources from the world.
    pub fn release(
        &mut self,
        genome: Genome,
        position: [f32; 2],
        count: usize,
    ) -> Result<(), String> {
        if count == 0
            || count > self.config.world.population_limit - self.agents.len()
            || position.iter().any(|x| !x.is_finite() || *x < 0.0)
        {
            return Err("invalid release population or position".into());
        }
        let hash = self.register_genome(genome.clone(), vec![])?;
        if self.genomes[&hash].exploit {
            return Err("exploit-suspect body cannot be released".into());
        }
        let mass = self.genomes[&hash].traits.mass;
        let per_n = (mass * self.config.world.nutrient_units_per_kg) as i64;
        let per_e = self.config.world.initial_energy;
        if self.cells.iter().map(|c| c.soil).sum::<i64>() < per_n * count as i64
            || self.cells.iter().map(|c| c.plant.energy).sum::<i64>() < per_e * count as i64
        {
            return Err("world lacks resources for this founding population".into());
        }
        let mut n = per_n * count as i64;
        let mut e = per_e * count as i64;
        for c in &mut self.cells {
            n -= take(&mut c.soil, n);
            e -= take(&mut c.plant.energy, e);
            if n == 0 && e == 0 {
                break;
            }
        }
        let extent = self.config.world.grid as f32 * self.config.world.cell_metres;
        let position = position.map(|v| v.rem_euclid(extent));
        for _ in 0..count {
            let id = self.next_id;
            self.next_id += 1;
            self.history.insert(
                id,
                LifeRecord {
                    id,
                    genome: hash,
                    born: self.tick,
                    died: None,
                    species: 0,
                    cause: None,
                    offspring: 0,
                    parents: vec![],
                },
            );
            self.agents.push(Agent {
                pheromone_guidance: None,
                infection: None,
                id,
                genome: hash,
                position,
                energy: per_e,
                nutrients: per_n,
                age: 0,
                health: 1.0,
                growth: 1.0,
                injury: 0.0,
                brain: genome.ethology.state(),
                outputs: vec![0.0; genome.ethology.neurons.len()],
                species: 0,
                parents: vec![],
                offspring: 0,
            });
        }
        Ok(())
    }
}
