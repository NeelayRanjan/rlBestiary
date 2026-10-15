use bestiary_core::genome;
use bestiary_sim::config::Config;
use bestiary_world::{forager_brain, predator_brain, scavenger_brain, World};
pub fn trophic(
    seed: u64,
    config: Config,
) -> Result<World, Box<dyn std::error::Error + Send + Sync>> {
    let counts = [32, 8, 4];
    let zoo = genome::zoo(&config.genome);
    let mut grazer = zoo[0].1.clone();
    grazer.ethology = forager_brain();
    grazer.life.maturity = config.reproduction.founder_maturity;
    grazer.life.senescence = config.reproduction.founder_senescence;
    grazer.life.clutch_size = 2;
    grazer.life.species_marker = [0.1; 4];
    let mut world = World::with_founder(seed, config, counts[0], grazer)?;
    for (index, brain, marker, count) in [
        (2, predator_brain(), 0.9, counts[1]),
        (3, scavenger_brain(), 0.5, counts[2]),
    ] {
        let mut genome = zoo[index].1.clone();
        genome.ethology = brain;
        genome.life.maturity = world.config.reproduction.founder_maturity;
        genome.life.senescence = world.config.reproduction.founder_senescence;
        genome.life.clutch_size = 2;
        genome.life.offspring_fraction = 0.5;
        genome.life.species_marker = [marker; 4];
        genome.life.plant_efficiency = 0.0;
        for founder in 0..count {
            let position = world.agents[(founder * 7 + index) % counts[0]].position;
            world.release(genome.clone(), position, 1)?;
        }
    }
    for (hash, entry) in &world.genomes {
        println!(
            "founder={} parts={} viability={} speed={} strike={} standing={} plant_efficiency={}",
            bestiary_core::hex(hash),
            entry.traits.parts,
            entry.viability,
            entry.traits.speed,
            entry.traits.strike_impulse,
            entry.traits.standing_cost,
            entry.genome.life.plant_efficiency
        );
        if entry.viability <= 0.0 || entry.exploit {
            return Err(
                "trophic founder must pass the physical viability and exploit gates".into(),
            );
        }
    }
    Ok(world)
}
