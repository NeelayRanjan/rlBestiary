use super::*;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pathogen {
    pub transmission: f32,
    pub virulence: f32,
    pub antigen: [f32; 4],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EcologyEvent {
    Predation { attacker: u64, victim: u64 },
    Death { creature: u64 },
    Geography,
    Shock { kind: u8 },
}
#[derive(Clone, Debug)]
pub struct Neighbor {
    pub index: usize,
    pub distance: f32,
    pub direction: [f32; 2],
    pub mass: f32,
    pub genome: [u8; 32],
}
impl World {
    pub fn neighbors(&self) -> Vec<Option<Neighbor>> {
        let cfg = &self.config.world;
        let mut cells = vec![vec![]; self.cells.len()];
        for (i, a) in self.agents.iter().enumerate() {
            if a.health > 0.0 {
                cells[self.cell_at(a.position)].push(i);
            }
        }
        let extent = cfg.grid as f32 * cfg.cell_metres;
        self.agents
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let cell = self.cell_at(a.position);
                let x = cell % cfg.grid;
                let z = cell / cfg.grid;
                let mut best: Option<Neighbor> = None;
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        let nx = (x as i32 + dx).rem_euclid(cfg.grid as i32) as usize;
                        let nz = (z as i32 + dz).rem_euclid(cfg.grid as i32) as usize;
                        for j in &cells[nz * cfg.grid + nx] {
                            if *j == i {
                                continue;
                            }
                            let b = &self.agents[*j];
                            let mut d =
                                [b.position[0] - a.position[0], b.position[1] - a.position[1]];
                            for v in &mut d {
                                if *v > extent * 0.5 {
                                    *v -= extent;
                                } else if *v < -extent * 0.5 {
                                    *v += extent;
                                }
                            }
                            let distance = libm::sqrtf(d[0] * d[0] + d[1] * d[1]);
                            if best.as_ref().is_none_or(|n| distance < n.distance) {
                                best = Some(Neighbor {
                                    index: *j,
                                    distance,
                                    direction: d.map(|x| x / distance.max(1e-5)),
                                    mass: self.genomes[&b.genome].traits.mass * b.growth,
                                    genome: b.genome,
                                });
                            }
                        }
                    }
                }
                best
            })
            .collect()
    }
    pub(crate) fn ecology_step(&mut self) {
        let cfg = &self.config.ecology;
        let wc = &self.config.world;
        let neighbors = self.neighbors();
        let old: Vec<_> = self.cells.iter().map(|c| c.pheromones).collect();
        for (i, cell) in self.cells.iter_mut().enumerate() {
            let x = i % wc.grid;
            let z = i / wc.grid;
            let around = [
                z * wc.grid + (x + 1) % wc.grid,
                z * wc.grid + (x + wc.grid - 1) % wc.grid,
                ((z + 1) % wc.grid) * wc.grid + x,
                ((z + wc.grid - 1) % wc.grid) * wc.grid + x,
            ];
            for channel in 0..4 {
                let mean = around.iter().map(|j| old[*j][channel]).sum::<f32>() * 0.25;
                cell.pheromones[channel] = ((1.0 - cfg.pheromone_diffusion) * old[i][channel]
                    + cfg.pheromone_diffusion * mean)
                    * (1.0 - cfg.pheromone_decay);
            }
        }
        for (i, neighbor) in neighbors.iter().enumerate() {
            if self.agents[i].health <= 0.0 {
                continue;
            }
            let hash = self.agents[i].genome;
            let entry = &self.genomes[&hash];
            let genes = &entry.genome.life;
            let position = self.agents[i].position;
            let cell = cell_at(position, wc);
            let outputs = self.agents[i].outputs.clone();
            let output = |n: usize| outputs.get(n).copied().unwrap_or(0.0);
            for channel in 0..4 {
                let amount = output(9 + channel).clamp(0.0, 1.0) * genes.pheromone_strength;
                let cost = (cfg.pheromone_cost as f32 * amount) as i64;
                let paid = take(&mut self.agents[i].energy, cost);
                self.dissipated += paid;
                if cost == 0 || paid == cost {
                    self.cells[cell].pheromones[channel] =
                        (self.cells[cell].pheromones[channel] + amount).min(1000.0);
                }
            }
            if let Some(neighbor) = neighbor {
                let j = neighbor.index;
                if output(6) > output(3).max(output(8))
                    && neighbor.distance <= entry.traits.reach
                    && self.agents[j].health > 0.0
                {
                    let paid = take(&mut self.agents[i].energy, cfg.attack_cost);
                    self.dissipated += paid;
                    if paid == cfg.attack_cost {
                        let mut rng =
                            bestiary_core::rng(self.seed, 0x4154544b, self.agents[i].id, self.tick);
                        let defender = &self.genomes[&self.agents[j].genome];
                        let armor = defender.traits.mass / defender.traits.volume.max(0.001);
                        let speed_factor =
                            (1.0 + entry.traits.speed) / (1.0 + defender.traits.speed);
                        let damage = cfg.attack_scale * entry.traits.strike_impulse * speed_factor
                            / (neighbor.mass.max(0.001) * (1.0 + armor));
                        if rng.gen::<f32>() >= cfg.refuge_fraction {
                            self.agents[j].health -= damage;
                            self.agents[j].injury =
                                (self.agents[j].injury + damage * cfg.injury_fraction).min(0.9);
                            self.ecological_events.push(EcologyEvent::Predation {
                                attacker: self.agents[i].id,
                                victim: self.agents[j].id,
                            });
                        }
                    }
                }
                if self.agents[i].infection.is_none() {
                    if let Some(strain) = self.agents[j].infection {
                        let p = &self.pathogens[strain];
                        let mut rng =
                            bestiary_core::rng(self.seed, 0x50415448, self.agents[i].id, self.tick);
                        if rng.gen::<f32>() < p.transmission * (1.0 - genes.immune_investment) {
                            self.agents[i].infection = Some(strain);
                        }
                    }
                }
            }
            if output(3) > output(8) {
                for carcass in &mut self.carcasses {
                    if cell_at(carcass.position, wc) != cell {
                        continue;
                    }
                    let reserve = (entry.traits.mass
                        * self.agents[i].growth
                        * wc.reserve_per_kg as f32) as i64;
                    let meal = take(
                        &mut carcass.energy,
                        wc.bite_energy.min((reserve - self.agents[i].energy).max(0)),
                    );
                    let nutrients = fraction(
                        carcass.nutrients,
                        meal as f32 / (carcass.energy + meal).max(1) as f32,
                    );
                    carcass.nutrients -= nutrients;
                    let needed = (entry.traits.mass
                        * (1.0 - self.agents[i].growth)
                        * wc.nutrient_units_per_kg) as i64;
                    let retained = nutrients.min(needed.max(0)).min(
                        (entry.traits.mass * wc.growth_per_tick * wc.nutrient_units_per_kg) as i64,
                    );
                    self.agents[i].nutrients += retained;
                    self.agents[i].growth = (self.agents[i].growth
                        + retained as f32
                            / (entry.traits.mass * wc.nutrient_units_per_kg).max(1.0))
                    .min(1.0);
                    self.cells[cell].soil += nutrients - retained;
                    let assimilated = fraction(meal, 1.0 - genes.plant_efficiency);
                    self.agents[i].energy += assimilated;
                    self.dissipated += meal - assimilated;
                    break;
                }
            }
            if let Some(strain) = self.agents[i].infection {
                let p = &self.pathogens[strain];
                let mismatch = p
                    .antigen
                    .iter()
                    .zip(genes.resistance)
                    .map(|(a, b)| (a - b).abs())
                    .sum::<f32>()
                    / 4.0;
                self.agents[i].health -= p.virulence * (1.0 - genes.immune_investment) * mismatch;
                let mut rng =
                    bestiary_core::rng(self.seed, 0x494d4d55, self.agents[i].id, self.tick);
                if rng.gen::<f32>() < genes.immune_investment * 0.01 {
                    self.agents[i].infection = None;
                }
            }
        }
        if self.tick % cfg.pathogen_epoch == 0 && !self.pathogens.is_empty() {
            let counts: Vec<_> = (0..self.pathogens.len())
                .map(|s| {
                    self.agents
                        .iter()
                        .filter(|a| a.infection == Some(s))
                        .count()
                })
                .collect();
            let best = (0..counts.len())
                .max_by_key(|i| (counts[*i], std::cmp::Reverse(*i)))
                .expect("strains");
            let worst = (0..counts.len())
                .min_by_key(|i| (counts[*i], *i))
                .expect("strains");
            let mut next = self.pathogens[best].clone();
            let mut rng = bestiary_core::rng(self.seed, 0x44524946, worst as u64, self.tick);
            for x in &mut next.antigen {
                *x = (*x + rng.gen_range(-cfg.antigen_drift..=cfg.antigen_drift)).clamp(0.0, 1.0);
            }
            next.virulence = (next.virulence
                + rng.gen_range(-cfg.antigen_drift..=cfg.antigen_drift) * 0.1)
                .clamp(0.0, 0.1);
            next.transmission = (next.transmission
                + rng.gen_range(-cfg.antigen_drift..=cfg.antigen_drift) * 0.1)
                .clamp(0.0, 1.0);
            self.pathogens[worst] = next;
            if !self.agents.is_empty() {
                let i = rng.gen_range(0..self.agents.len());
                self.agents[i].infection = Some(worst);
            }
        }
        if self.tick % cfg.tectonic_epoch == 0 {
            self.sea_level = wc.water_level
                + cfg.sea_amplitude
                    * libm::sinf(self.tick as f32 / (cfg.tectonic_epoch as f32 * 10.0));
            for (i, c) in self.cells.iter_mut().enumerate() {
                c.height = (c.height + cfg.uplift * libm::sinf(i as f32 * 0.1)).clamp(-2.0, 2.0);
            }
            self.ecological_events.push(EcologyEvent::Geography);
            let mut rng = bestiary_core::rng(self.seed, 0x53484f43, 0, self.tick);
            if rng.gen::<f32>() < cfg.shock_probability {
                let kind = rng.gen_range(0..3);
                self.ecological_events.push(EcologyEvent::Shock { kind });
                match kind {
                    0 => {
                        for a in &mut self.agents {
                            a.health -= rng.gen_range(0.0..0.4);
                        }
                    }
                    1 => {
                        for cell in &mut self.cells {
                            cell.moisture = (cell.moisture * 0.8).max(0.1);
                        }
                    }
                    _ => {
                        if !self.pathogens.is_empty() {
                            for a in &mut self.agents {
                                a.infection = Some(rng.gen_range(0..self.pathogens.len()));
                            }
                        }
                    }
                }
            }
        }
    }
}
