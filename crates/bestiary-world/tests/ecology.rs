use bestiary_archive::store::Persistable;
use bestiary_core::genome::Genome;
use bestiary_sim::config::Config;
use bestiary_world::{
    forager_brain, predator_brain, scavenger_brain, Carcass, EcologyEvent, World,
};

#[test]
fn depleted_carcass_does_not_block_edible_carcass_and_conserves_resources() {
    let mut config = Config::bundled();
    config.world.grid = 4;
    config.world.seconds_per_tick = 1.0;
    config.world.initial_energy = 10_000;
    config.world.bite_energy = 1_000_000;
    config.world.decision_interval = 1000;
    config.ecology.pathogen_count = 0;
    let mut genome = Genome::starter(&config.genome);
    genome.ethology = scavenger_brain();
    genome.life.plant_efficiency = 0.0;
    let mut world = World::with_founder(44, config, 1, genome).unwrap();
    world.tick = 2;
    world.agents[0].outputs = vec![0.0; 13];
    world.agents[0].outputs[3] = 1.0;
    let position = world.agents[0].position;
    let cell = world.cell_at(position);
    let meal = world.config.world.bite_energy;
    // Move existing resources into two carcasses; a depleted carcass may still
    // contain nutrients until decay completes, so it remains in the world.
    world.cells[cell].soil -= 1000;
    world.cells[cell].plant.energy -= 4 * meal;
    world.cells[cell].plant.nutrients -= 4000;
    world.carcasses = vec![
        Carcass {
            position,
            energy: 0,
            nutrients: 1000,
        },
        Carcass {
            position,
            energy: 4 * meal,
            nutrients: 4000,
        },
    ];
    assert!(world.conserved());
    let mut edible_first = world.clone();
    edible_first.carcasses.reverse();
    world.step();
    edible_first.step();
    assert!(world.conserved() && edible_first.conserved());
    assert_eq!(world.agents.len(), 1);
    assert!(world.carcasses[0].nutrients > 0);
    assert_eq!(world.carcasses[0].energy, 0);
    assert!(world.agents[0].energy >= meal);
    assert_eq!(world.agents[0].energy, edible_first.agents[0].energy);
    assert_eq!(world.agents[0].nutrients, edible_first.agents[0].nutrients);
    assert_eq!(world.totals(), edible_first.totals());
}

#[test]
fn predation_injury_and_scavenging_preserve_resources() {
    let mut config = Config::bundled();
    config.world.grid = 4;
    config.world.decision_interval = 1000;
    config.ecology.refuge_fraction = 0.0;
    config.ecology.attack_scale = 1000.0;
    let mut world = World::new(42, config, 2).unwrap();
    world.tick = 2;
    world.agents[1].position = world.agents[0].position;
    world.agents[0].outputs = vec![0.0; 13];
    world.agents[0].outputs[6] = 1.0;
    let before = world.agents[1].health;
    world.step();
    assert!(world.conserved());
    assert!(world.agents[1].health < before || world.agents.len() < 2);
    assert!(world
        .ecological_events
        .iter()
        .any(|e| matches!(e, EcologyEvent::Predation { .. })));
    if world.agents.len() > 1 {
        world.agents[1].health = -1.0;
    }
    world.agents[0].outputs[6] = 0.0;
    world.agents[0].outputs[3] = 1.0;
    for _ in 0..20 {
        world.step();
        assert!(world.conserved());
    }
    assert!(world.agents.len() < 2);
}
#[test]
fn pheromones_diffuse_decay_and_infections_and_geography_repeat() {
    let mut config = Config::bundled();
    config.world.grid = 4;
    config.ecology.tectonic_epoch = 10;
    config.ecology.pathogen_epoch = 10;
    let mut a = World::new(1, config, 2).unwrap();
    a.cells[0].pheromones[0] = 10.0;
    a.agents[0].infection = Some(0);
    let original_height = a.cells[1].height;
    let mut b = a.clone();
    for _ in 0..100 {
        a.step();
        b.step();
        assert!(a.conserved());
    }
    assert_eq!(a.state_hash(), b.state_hash());
    assert!(a.cells[1].pheromones[0] > 0.0);
    assert_ne!(a.cells[1].height, original_height);
    assert!(a
        .cells
        .iter()
        .all(|c| c.pheromones.iter().all(|x| x.is_finite() && *x >= 0.0)));
}
#[test]
fn ecology_config_negative_cases() {
    let mut c = Config::bundled().ecology;
    c.pathogen_epoch = 0;
    assert!(c.validate().is_err());
    c = Config::bundled().ecology;
    c.pheromone_decay = 1.1;
    assert!(c.validate().is_err());
    for toxin_damage in [-0.1, 1.1, f32::NAN, f32::INFINITY] {
        c = Config::bundled().ecology;
        c.toxin_damage = toxin_damage;
        assert!(c.validate().is_err());
    }
}

fn herbivore_toxin_fixture(toxicity: f32, immune_investment: f32) -> World {
    let mut config = Config::bundled();
    config.world.grid = 4;
    config.world.initial_energy = 200_000_000;
    config.world.bite_energy = 1_000_000;
    config.world.seconds_per_tick = 60.0;
    config.world.decision_interval = 10_000;
    config.world.plant_epoch = 10_000;
    config.reproduction.epoch = 10_000;
    config.ecology.pathogen_count = 0;
    config.ecology.shock_probability = 0.0;
    let mut genome = Genome::starter(&config.genome);
    genome.ethology = forager_brain();
    genome.life.immune_investment = immune_investment;
    let mut world = World::with_founder(71, config, 1, genome).unwrap();
    world.tick = 1;
    world.agents[0].outputs = vec![0.0; 13];
    world.agents[0].outputs[3] = 1.0;
    for cell in &mut world.cells {
        cell.plant.toxicity = toxicity;
        cell.plant.toughness = 0.0;
    }
    world
}

#[test]
fn toxic_plants_impose_survival_pressure_resisted_by_immune_investment() {
    let mut clean = herbivore_toxin_fixture(0.0, 0.0);
    let mut susceptible = herbivore_toxin_fixture(1.0, 0.0);
    let mut resistant = herbivore_toxin_fixture(1.0, 0.9);
    for _ in 0..60 {
        for world in [&mut clean, &mut susceptible, &mut resistant] {
            world.step();
            assert!(world.conserved());
        }
    }
    assert_eq!(clean.agents.len(), 1, "clean-fed control must survive");
    assert!(
        susceptible.agents.is_empty(),
        "toxin must impose a fitness cost"
    );
    assert_eq!(
        resistant.agents.len(),
        1,
        "resistance must prevent toxic death"
    );
    assert!(resistant.agents[0].health > 0.9);
}

#[test]
fn toxin_damage_depends_on_food_ingested_and_is_not_healed_in_the_same_tick() {
    let mut toxic = herbivore_toxin_fixture(1.0, 0.0);
    let mut no_food = toxic.clone();
    for cell in &mut no_food.cells {
        no_food.dissipated += cell.plant.energy;
        cell.plant.energy = 0;
        cell.soil += cell.plant.nutrients;
        cell.plant.nutrients = 0;
    }
    no_food.config.world.solar_per_cell = 0;
    toxic.step();
    no_food.step();
    assert!(toxic.conserved() && no_food.conserved());
    assert_eq!(
        no_food.agents[0].health, 1.0,
        "an empty bite has no toxin dose"
    );
    assert!(toxic.agents[0].health < 1.0);
    assert_eq!(
        toxic.agents[0].health,
        1.0 - toxic.config.ecology.toxin_damage
    );
}

#[test]
fn herbivore_resistance_series_excludes_other_diet_tiers() {
    let mut world = herbivore_toxin_fixture(0.0, 0.2);
    let mut carnivore = world.genomes[&world.agents[0].genome].genome.clone();
    carnivore.life.plant_efficiency = 0.1;
    carnivore.life.immune_investment = 0.9;
    world.release(carnivore, [1.0, 1.0], 2).unwrap();
    let sample = world.sample();
    assert!(sample.trophic_biomass[0] > 0.0 && sample.trophic_biomass[2] > 0.0);
    assert!((sample.herbivore_resistance - 0.2).abs() < 1e-6);
    assert!(world.conserved());
}

#[test]
fn inherited_predator_and_scavenger_networks_express_distinct_feeding_actions() {
    let predator = predator_brain();
    let mut sensors = vec![0.0; 20];
    sensors[0] = 0.1;
    sensors[11] = 0.0;
    let mut hunting = predator.state();
    predator.step(&mut hunting, &sensors, &[], &[], 1.0);
    assert!(hunting.values[4] > hunting.values[3]);
    assert!(hunting.values[6] > hunting.values[3]);
    sensors[11] = 1.0;
    let mut kin = predator.state();
    predator.step(&mut kin, &sensors, &[], &[], 1.0);
    assert!(kin.values[6] < kin.values[3]);
    sensors[0] = 1.0;
    predator.step(&mut kin, &sensors, &[], &[], 2.0);
    assert!(kin.values[7] > kin.values[8]);
    let scavenger = scavenger_brain();
    let mut feeding = scavenger.state();
    scavenger.step(&mut feeding, &sensors, &[], &[], 1.0);
    assert!(feeding.values[3] > feeding.values[6].max(feeding.values[8]));
    assert!(feeding.values[2] > 0.0);
}

#[test]
fn inherited_predator_brain_hunts_using_measured_body_traits() {
    let mut config = Config::bundled();
    config.world.grid = 4;
    config.world.seconds_per_tick = 60.0;
    config.world.decision_interval = 1;
    config.reproduction.epoch = 10_000;
    let zoo = bestiary_core::genome::zoo(&config.genome);
    let mut grazer = zoo[0].1.clone();
    grazer.ethology = forager_brain();
    let mut predator = zoo[2].1.clone();
    predator.ethology = predator_brain();
    predator.life.plant_efficiency = 0.0;
    predator.life.species_marker = [1.0; 4];
    let mut world = World::with_founder(42, config, 1, grazer).unwrap();
    world.agents[0].position = [20.0, 20.0];
    world.release(predator, [20.5, 20.0], 1).unwrap();
    let mut strikes = 0;
    for _ in 0..100 {
        world.step();
        assert!(world.conserved());
        strikes += world
            .ecological_events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    EcologyEvent::Predation {
                        attacker: 1,
                        victim: 0
                    }
                )
            })
            .count();
    }
    assert!(strikes > 0);
    assert_eq!(world.history[&0].cause.as_deref(), Some("predation"));
    assert!(world.agents.iter().any(|agent| agent.id == 1));
}

#[test]
fn species_similarity_sensor_recognizes_mutant_kin_and_distinguishes_unrelated_genomes() {
    use bestiary_core::brain::{Activation, Connection, Input};
    let mut config = Config::bundled();
    config.world.grid = 4;
    config.world.decision_interval = 1;
    let mut genome = Genome::starter(&config.genome);
    genome.ethology = forager_brain();
    genome.ethology.neurons[8].activation = Activation::Abs;
    genome.ethology.neurons[8].bias = 0.0;
    genome.ethology.neurons[8].inputs = vec![Connection {
        input: Input::Sensor(11),
        weight: 1.0,
        learning_rate: 0.0,
        decay: 0.0,
    }];
    let mut mutant = genome.clone();
    mutant.bones[0].color[0] += 0.01;
    assert_ne!(genome.hash(), mutant.hash());
    let mut unrelated = mutant.clone();
    unrelated.life.species_marker = [1.0; 4];
    let mut results = vec![];
    for neighbor in [mutant, unrelated] {
        let mut world = World::with_founder(9, config.clone(), 1, genome.clone()).unwrap();
        world.agents[0].position = [20.0, 20.0];
        world.release(neighbor, [20.5, 20.0], 1).unwrap();
        let mut repeated = world.clone();
        world.step();
        repeated.step();
        assert_eq!(world.state_hash(), repeated.state_hash());
        assert!(world.conserved());
        results.push(world.agents[0].brain.values[8]);
    }
    assert!(results[0] > 0.9, "mutant kin similarity={}", results[0]);
    assert_eq!(results[1], 0.0);
}

#[test]
fn nearest_neighbors_match_distance_oracle_and_wrap_at_world_edge() {
    let mut c = Config::bundled();
    c.world.grid = 4;
    let mut w = World::new(10, c, 3).unwrap();
    w.agents[0].position = [1.0, 1.0];
    w.agents[1].position = [127.0, 1.0];
    w.agents[2].position = [6.0, 1.0];
    let n = w.neighbors();
    assert_eq!(n[0].as_ref().unwrap().index, 1);
    assert_eq!(n[0].as_ref().unwrap().distance, 2.0);
    assert_eq!(n[0].as_ref().unwrap().direction, [-1.0, 0.0]);
    assert_eq!(n[1].as_ref().unwrap().index, 0);
    assert_eq!(n[2].as_ref().unwrap().index, 0);
}
