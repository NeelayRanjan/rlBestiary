//! Bounded diagnostic for mixed founders; it reports evidence and does not assert long-run acceptance.
use bestiary_archive::store::{Persistable, Store};
use bestiary_core::genome;
use bestiary_sim::config::Config;
use bestiary_world::{forager_brain, predator_brain, scavenger_brain, EcologyEvent, World};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() < 3 {
        return Err(
            "usage: trophic CONFIG SEED TICKS [NEW_OUTPUT_DB] [GRAZERS PREDATORS SCAVENGERS]"
                .into(),
        );
    }
    let config = Config::parse(&std::fs::read_to_string(&args[0])?)?;
    let seed = args[1].parse::<u64>()?;
    let ticks = args[2].parse::<u64>()?;
    let counts = [32usize, 8, 4]
        .into_iter()
        .enumerate()
        .map(|(index, default)| {
            args.get(index + 4)
                .map_or(Ok(default), |value| value.parse::<usize>())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if counts.iter().any(|count| *count < 2 || count % 2 != 0) {
        return Err("founder counts must be positive pairs".into());
    }
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
    let mut store = args
        .get(3)
        .map(|path| Store::create(std::path::Path::new(path)))
        .transpose()?;
    let mut predation = 0u64;
    let mut first_missing_carnivore = None;
    for _ in 0..ticks {
        world.step();
        predation += world
            .ecological_events
            .iter()
            .filter(|event| matches!(event, EcologyEvent::Predation { .. }))
            .count() as u64;
        if let Some(store) = &store {
            store.events(world.tick, &world.ecological_events)?;
        }
        if world.tick % 1000 == 0 || world.tick == ticks {
            assert!(world.conserved(), "conservation at tick {}", world.tick);
            let sample = world.sample();
            let mut births = [0usize; 3];
            let mut living = [0usize; 3];
            let mut lineages = std::collections::BTreeMap::new();
            let mut lineage_living = [0usize; 3];
            let mut lineage_births = [0usize; 3];
            for life in world.history.values() {
                let lineage = if let Some(parent) = life.parents.first() {
                    lineages[parent]
                } else if life.id < counts[0] as u64 {
                    0
                } else if life.id < (counts[0] + counts[1]) as u64 {
                    1
                } else {
                    2
                };
                lineages.insert(life.id, lineage);
                let efficiency = world.genomes[&life.genome].genome.life.plant_efficiency;
                let tier = if efficiency >= 0.65 {
                    0
                } else if efficiency <= 0.35 {
                    2
                } else {
                    1
                };
                if !life.parents.is_empty() {
                    births[tier] += 1;
                    lineage_births[lineage] += 1;
                }
                if life.died.is_none() {
                    living[tier] += 1;
                    lineage_living[lineage] += 1;
                }
            }
            if living[2] == 0 && first_missing_carnivore.is_none() {
                first_missing_carnivore = Some(world.tick);
            }
            println!(
                "tick={} diet_live={:?} diet_births={:?} lineage_live={:?} lineage_births={:?} species={} predation={} coverage={} toxicity={} resistance={} conserved=true",
                world.tick, living, births, lineage_living, lineage_births, sample.species, predation, sample.coverage,
                sample.plant_toxicity, sample.herbivore_resistance
            );
            if let Some(store) = &mut store {
                store.save(&world)?;
            }
            for entry in world.genomes.values_mut() {
                entry.replays.clear();
            }
        }
    }
    println!("first_missing_carnivore={first_missing_carnivore:?}");
    Ok(())
}
