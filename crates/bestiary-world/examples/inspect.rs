use bestiary_archive::store::{Store, TimeSample};
use bestiary_world::{EcologyEvent, World};

fn normalized_variance(sample: &TimeSample) -> f32 {
    sample
        .trait_variance
        .iter()
        .zip(&sample.mean_traits)
        .map(|(variance, mean)| variance / (1.0 + mean * mean))
        .sum()
}

fn print_sample(sample: &TimeSample, world: &World) {
    let largest_living_body = world
        .history
        .values()
        .filter(|life| life.born <= sample.tick && life.died.is_none_or(|tick| tick > sample.tick))
        .map(|life| world.genomes[&life.genome].traits.parts)
        .max()
        .unwrap_or(0);
    println!(
        "sample_tick={} population={} species={} coverage={} largest_living_body={} normalized_trait_variance={} trophic_biomass={:?} plant_toxicity={} mean_immune_investment={}",
        sample.tick,
        sample.population,
        sample.species,
        sample.coverage,
        largest_living_body,
        normalized_variance(sample),
        sample.trophic_biomass,
        sample.plant_toxicity,
        sample.herbivore_resistance
    );
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let path = std::env::args().nth(1).ok_or("pass world database")?;
    let store = Store::open(std::path::Path::new(&path), true)?;
    let w: World = store.load()?;
    let events = store.event_history::<EcologyEvent>()?;
    let first_fragmentation = events.iter().find_map(|(tick, event)| {
        matches!(event, EcologyEvent::Fragmentation { .. }).then_some(*tick)
    });
    let species = w
        .registry
        .species
        .values()
        .filter(|s| s.population > 0)
        .count();
    let splits = w
        .registry
        .species
        .values()
        .filter(|s| s.parent.is_some() && first_fragmentation.is_some_and(|t| s.born > t))
        .count();
    let max_parts = w
        .genomes
        .values()
        .filter(|g| !g.exploit)
        .map(|g| g.traits.parts)
        .max();
    println!("tick={} population={} species={} splits_after_fragmentation={} first_fragmentation={:?} max_valid_parts={:?} conserved={} mutations={}",w.tick,w.agents.len(),species,splits,first_fragmentation,max_parts,w.conserved(),w.mutations.len());
    let founding_parts = w
        .history
        .values()
        .filter(|life| life.born == 0 && life.parents.is_empty())
        .map(|life| w.genomes[&life.genome].traits.parts)
        .max()
        .unwrap_or(0);
    let archive_parts = w
        .archive
        .cells
        .values()
        .map(|elite| elite.traits.parts)
        .max()
        .unwrap_or(0);
    let living_species_parts = w
        .agents
        .iter()
        .filter(|agent| agent.species != 0)
        .map(|agent| w.genomes[&agent.genome].traits.parts)
        .max()
        .unwrap_or(0);
    println!(
        "archive_cells={} coverage={} founding_parts={} max_archive_parts={} max_living_species_parts={}",
        w.archive.cells.len(),
        w.archive.coverage(&w.config.archive),
        founding_parts,
        archive_parts,
        living_species_parts
    );
    let samples = store.samples()?;
    let first_coverage_and_coexistence = samples
        .iter()
        .find(|sample| sample.coverage >= 0.15 && sample.species >= 3);
    if let Some(sample) = first_coverage_and_coexistence {
        println!("first_saved_coverage_and_coexistence:");
        print_sample(sample, &w);
    } else {
        println!("first_saved_coverage_and_coexistence=none");
    }
    for tick in [750_000, 1_000_000] {
        if let Some(sample) = samples.iter().find(|sample| sample.tick == tick) {
            print_sample(sample, &w);
        }
    }
    for sample in samples.iter().rev().take(5).rev() {
        print_sample(sample, &w);
    }
    let trophic_samples = samples
        .iter()
        .filter(|sample| sample.trophic_biomass.iter().all(|mass| *mass > 0.0))
        .count();
    println!(
        "samples_with_all_three_diet_tiers={trophic_samples}/{}",
        samples.len()
    );
    let coverage_tick = first_coverage_and_coexistence.map_or(w.tick, |sample| sample.tick);
    for species in w.registry.species.values().filter(|species| {
        w.genomes[&species.founder].traits.parts > founding_parts
            && species.born <= coverage_tick
            && species.extinct.is_none_or(|tick| tick > coverage_tick)
    }) {
        println!(
            "larger_species={} born={} extinct={:?} founder_parts={} population={}",
            species.id,
            species.born,
            species.extinct,
            w.genomes[&species.founder].traits.parts,
            species.population
        );
    }
    if std::env::args().any(|arg| arg == "--recombine") {
        for (cell, elite) in &w.archive.cells {
            let evaluation =
                bestiary_sim::combine::evaluate(&w.genomes[&elite.genome].genome, &w.config)?;
            if elite.traits != evaluation.traits || evaluation.exploit_suspect {
                return Err(format!("archive cell {cell:?} failed exact re-combination").into());
            }
        }
        println!(
            "exact_archive_recombinations_passed={}",
            w.archive.cells.len()
        );
    }
    Ok(())
}
