use bestiary_sim::config::Config;
use bestiary_world::{EcologyEvent, World};
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
}
