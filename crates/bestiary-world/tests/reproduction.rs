use bestiary_archive::store::Store;
use bestiary_core::genome::Genome;
use bestiary_sim::config::Config;
use bestiary_world::{forager_brain, World};
#[test]
fn sexual_births_and_sqlite_resume_conserve_resources() {
    let mut config = Config::bundled();
    config.world.grid = 4;
    config.reproduction.epoch = 1;
    config.reproduction.mutation_probability = 0.0;
    config.reproduction.species_epoch = 1;
    config.reproduction.species_min_duration = 2;
    config.world.population_limit = 16;
    let mut g = Genome::starter(&config.genome);
    g.ethology = forager_brain();
    g.life.maturity = 2;
    g.life.senescence = 2000;
    g.life.clutch_size = 2;
    let mut world = World::with_founder(42, config, 2, g).unwrap();
    world.agents[1].position = world.agents[0].position;
    for _ in 0..30 {
        world.step();
        assert!(world.conserved());
    }
    assert!(world.history.len() > 2);
    assert!(!world.registry.species.is_empty());
    assert!(world
        .history
        .values()
        .filter(|r| r.id >= 2)
        .all(|r| r.parents.len() == 2));
    let path = std::env::temp_dir().join(format!("bestiary-records-{}.db", std::process::id()));
    let mut store = Store::create(&path).unwrap();
    store.save(&world).unwrap();
    let mut resumed: World = store.load().unwrap();
    assert_eq!(world.state_hash(), resumed.state_hash());
    for g in resumed.genomes.values_mut() {
        g.replays.clear();
    }
    assert_eq!(world.state_hash(), resumed.state_hash());
    for _ in 0..5 {
        world.step();
        resumed.step();
    }
    assert_eq!(world.state_hash(), resumed.state_hash());
    assert!(resumed.conserved());
    assert!(store.counts().unwrap()["creature"].as_u64().unwrap() > 2);
    assert_eq!(store.samples().unwrap().len(), 1);
    assert_eq!(
        store.counts().unwrap()["archive_cell"],
        world.archive.cells.len()
    );
    drop(store);
    std::fs::remove_file(path).unwrap();
}
#[test]
fn reproduction_config_rejects_invalid_values() {
    let mut c = Config::bundled().reproduction;
    c.compatibility_threshold = 0.0;
    assert!(c.validate().is_err());
}

#[test]
fn senescence_cannot_be_cancelled_by_healing() {
    let mut c = Config::bundled();
    c.world.grid = 4;
    c.reproduction.epoch = 10000;
    let mut g = Genome::starter(&c.genome);
    g.ethology = forager_brain();
    g.life.maturity = 2;
    g.life.senescence = 10;
    let mut w = World::with_founder(1, c, 2, g).unwrap();
    for _ in 0..20 {
        w.step();
        assert!(w.conserved());
    }
    assert!(w.agents.is_empty());
    assert!(w.history.values().all(|h| h.died.is_some()));
}

#[test]
fn disconnected_terrestrial_mates_cannot_reproduce() {
    let mut c = Config::bundled();
    c.world.grid = 8;
    c.reproduction.epoch = 1;
    c.world.decision_interval = 10000;
    let mut w = World::new(1, c, 2).unwrap();
    for cell in &mut w.cells {
        cell.height = -1.0;
    }
    w.cells[0].height = 1.0;
    w.cells[2].height = 1.0;
    w.agents[0].position = [1.0, 1.0];
    w.agents[1].position = [65.0, 1.0];
    for a in &mut w.agents {
        a.age = 11000;
        a.outputs = vec![0.0; 13];
        a.outputs[7] = 1.0;
    }
    w.tick = 2;
    assert_eq!(w.land_components(), 2);
    w.step();
    assert_eq!(w.history.len(), 2);
    assert!(w.conserved());
}

fn mating_fixture(population: usize) -> World {
    let mut config = Config::bundled();
    config.world.grid = 4;
    config.world.initial_energy = 500_000_000;
    config.world.decision_interval = 10_000;
    config.reproduction.epoch = 1;
    config.reproduction.mutation_probability = 0.0;
    config.reproduction.compatibility_threshold = 0.04;
    let mut genome = Genome::starter(&config.genome);
    genome.ethology = forager_brain();
    genome.life.maturity = 2;
    let mut world = World::with_founder(42, config, population, genome).unwrap();
    for cell in &mut world.cells {
        cell.height = 1.0;
    }
    world.tick = 100;
    world
}

fn ready_to_mate(world: &mut World) {
    for agent in &mut world.agents {
        agent.age = 3;
        agent.outputs = vec![0.0; 13];
        agent.outputs[7] = 1.0;
    }
}

#[test]
fn eligible_compatible_mate_is_not_blocked_by_nearer_incompatible_neighbor() {
    let mut world = mating_fixture(2);
    world.agents[0].position = [10.0, 10.0];
    world.agents[1].position = [12.0, 10.0];
    let mut incompatible = world.genomes[&world.agents[0].genome].genome.clone();
    incompatible.life.species_marker = [1.0; 4];
    world.release(incompatible, [10.1, 10.0], 1).unwrap();
    ready_to_mate(&mut world);
    world.step();
    assert!(world.conserved());
    let children = world
        .history
        .values()
        .filter(|life| !life.parents.is_empty())
        .collect::<Vec<_>>();
    assert!(!children.is_empty());
    assert!(children.iter().all(|life| life.parents == [0, 1]));
}

#[test]
fn eligible_mates_choose_distance_then_entity_id_with_deterministic_ties() {
    let mut world = mating_fixture(3);
    world.agents[0].position = [10.0, 10.0];
    world.agents[1].position = [12.0, 10.0];
    world.agents[2].position = [8.0, 10.0];
    ready_to_mate(&mut world);
    world.agents.swap(1, 2);
    world.step();
    assert!(world.conserved());
    let child = world
        .history
        .values()
        .find(|life| !life.parents.is_empty())
        .unwrap();
    assert_eq!(child.parents, [0, 1]);
}

#[test]
fn eligible_mates_obey_radius_across_toroidal_world_boundary() {
    let mut world = mating_fixture(3);
    world.config.reproduction.mate_radius = 3.0;
    world.agents[0].position = [1.0, 1.0];
    world.agents[1].position = [127.0, 1.0];
    world.agents[2].position = [6.0, 1.0];
    ready_to_mate(&mut world);
    world.step();
    assert!(world.conserved());
    let children = world
        .history
        .values()
        .filter(|life| !life.parents.is_empty())
        .collect::<Vec<_>>();
    assert!(!children.is_empty());
    assert!(children.iter().all(|life| life.parents == [0, 1]));
}

#[test]
#[ignore = "250k-tick diversification acceptance; run with --release --ignored"]
fn three_species_coexist_and_phylogeny_splits_after_tectonic_fragmentation() {
    let c = Config::parse(include_str!("../../../config/demo.ron")).unwrap();
    let mut w = World::new(42, c, 24).unwrap();
    let mut store = std::env::var("BESTIARY_DEMO_OUT")
        .ok()
        .map(|p| Store::create(std::path::Path::new(&p)).unwrap());
    let mut fragmented = None;
    for _ in 0..250_000 {
        w.step();
        if let Some(store) = &store {
            store.events(w.tick, &w.ecological_events).unwrap();
        }
        if fragmented.is_none()
            && w.ecological_events
                .iter()
                .any(|e| matches!(e, bestiary_world::EcologyEvent::Fragmentation { .. }))
        {
            fragmented = Some(w.tick);
        }
        if w.tick % 1000 == 0 {
            assert!(w.conserved());
        }
        if w.tick % 10_000 == 0 {
            let species = w
                .registry
                .species
                .values()
                .filter(|s| s.population > 0)
                .count();
            println!(
                "tick={} population={} species={} fragmentation={fragmented:?}",
                w.tick,
                w.agents.len(),
                species
            );
            if let Some(store) = &mut store {
                store.save(&w).unwrap();
            }
            for g in w.genomes.values_mut() {
                g.replays.clear();
            }
        }
    }
    assert!(
        w.registry
            .species
            .values()
            .filter(|s| s.population > 0)
            .count()
            >= 3
    );
    let split = fragmented.expect("terrain must actually fragment");
    assert!(w
        .registry
        .species
        .values()
        .any(|s| s.parent.is_some() && s.born > split));
    assert!(w.history.values().any(|h| h.died.is_some()));
    assert!(!w.mutations.is_empty());
    let coverage = w.archive.coverage(&w.config.archive);
    let max_parts = w
        .archive
        .cells
        .values()
        .map(|e| e.traits.parts)
        .max()
        .unwrap();
    println!(
        "archive={} coverage={coverage:.4} max_parts={max_parts}",
        w.archive.cells.len()
    );
    for elite in w.archive.cells.values() {
        let evaluation =
            bestiary_sim::combine::evaluate(&w.genomes[&elite.genome].genome, &w.config).unwrap();
        assert_eq!(elite.traits, evaluation.traits);
        assert!(!evaluation.exploit_suspect);
    }
    assert!(max_parts > 7);
    assert!(
        coverage >= 0.15,
        "coverage {coverage} is below the fixed-grid floor"
    );
}

#[test]
fn optional_archive_reseeding_is_off_by_default_and_conserves_resources() {
    let mut c = Config::bundled();
    assert_eq!(c.archive.reseed_interval, 0);
    c.world.grid = 4;
    c.world.population_limit = 16;
    c.archive.reseed_interval = 1;
    let mut w = World::new(1, c, 2).unwrap();
    w.step();
    assert_eq!(w.agents.len(), 4);
    assert!(w.conserved());
    assert_eq!(w.archive.cells.len(), 1);
}
