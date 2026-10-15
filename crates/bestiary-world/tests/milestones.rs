use bestiary_core::brain::{Activation, Connection, Input};
use bestiary_sim::config::Config;
use bestiary_world::{EcologyEvent, World};

fn hunting_world(gradient: bool, depends_on_pheromones: bool) -> World {
    let mut config = Config::bundled();
    config.world.grid = 4;
    config.world.decision_interval = 10;
    config.ecology.refuge_fraction = 0.0;
    config.ecology.pathogen_count = 0;
    let mut world = World::new(42, config, 2).unwrap();
    let hash = world.agents[0].genome;
    let genome = world.genomes.get_mut(&hash).unwrap();
    // Deliberately isolated attack circuit: the trail flips attack over rest.
    for neuron in &mut genome.genome.ethology.neurons {
        neuron.bias = -100.0;
        neuron.activation = Activation::Sigmoid;
        neuron.inputs.clear();
    }
    genome.genome.ethology.neurons[3].bias = 0.0;
    genome.genome.ethology.neurons[8].bias = 0.0;
    let attack = &mut genome.genome.ethology.neurons[6];
    attack.bias = if depends_on_pheromones { -1.0 } else { 1.0 };
    if depends_on_pheromones {
        attack.inputs.push(Connection {
            input: Input::Sensor(5),
            weight: 2.0,
            learning_rate: 0.0,
            decay: 0.0,
        });
    }
    genome.traits.strike_impulse = 1.0;
    let brain = genome.genome.ethology.state();
    world.agents[0].brain = brain.clone();
    world.agents[1].brain = brain;
    world.agents[1].position = world.agents[0].position;
    if gradient {
        let cell = world.cell_at(world.agents[0].position);
        let grid = world.config.world.grid;
        let right = (cell / grid) * grid + (cell % grid + 1) % grid;
        world.cells[right].pheromones[0] = 2.0 * world.config.world.cell_metres;
    }
    world
}

#[test]
fn first_pheromone_guided_hunt_requires_causal_decision_and_real_damaging_predation() {
    let mut guided = hunting_world(true, true);
    let mut unguided_instrumentation = guided.clone();
    unguided_instrumentation.first_pheromone_guided_hunt = true;
    let victim_health = guided.agents[1].health;
    guided.step();
    unguided_instrumentation.step();
    assert!(guided.agents[1].health < victim_health);
    assert!(guided.first_pheromone_guided_hunt);
    assert!(guided.ecological_events.iter().any(|event| matches!(
        event,
        EcologyEvent::FirstPheromoneGuidedHunt {
            attacker: 0,
            victim: 1,
            decision_tick: 0,
            pheromone_channels: 1
        }
    )));
    assert!(guided.ecological_events.iter().any(|event| matches!(
        event,
        EcologyEvent::FirstPredation {
            attacker: 0,
            victim: 1
        }
    )));
    assert!(guided.conserved());
    for (actual, control) in guided.agents.iter().zip(&unguided_instrumentation.agents) {
        assert_eq!(actual.brain, control.brain);
        assert_eq!(actual.outputs, control.outputs);
        assert_eq!(actual.energy, control.energy);
        assert_eq!(actual.health, control.health);
        assert_eq!(actual.position, control.position);
    }
    let mut restored: World =
        postcard::from_bytes(&postcard::to_allocvec(&guided).unwrap()).unwrap();
    guided.step();
    restored.step();
    assert_eq!(guided.state_hash(), restored.state_hash());
    assert!(!restored.ecological_events.iter().any(|event| matches!(
        event,
        EcologyEvent::FirstPheromoneGuidedHunt { .. } | EcologyEvent::FirstPredation { .. }
    )));

    for (gradient, depends) in [(false, true), (true, false), (false, false)] {
        let mut world = hunting_world(gradient, depends);
        world.step();
        assert!(!world.first_pheromone_guided_hunt);
        assert!(!world
            .ecological_events
            .iter()
            .any(|event| matches!(event, EcologyEvent::FirstPheromoneGuidedHunt { .. })));
        assert!(world.conserved());
    }
    let mut zero_damage = hunting_world(true, true);
    let hash = zero_damage.agents[0].genome;
    zero_damage
        .genomes
        .get_mut(&hash)
        .unwrap()
        .traits
        .strike_impulse = 0.0;
    zero_damage.step();
    assert!(!zero_damage.first_pheromone_guided_hunt);
    assert!(!zero_damage.first_predation);
    let mut refuge = hunting_world(true, true);
    refuge.config.ecology.refuge_fraction = 1.0;
    refuge.step();
    assert!(!refuge.first_pheromone_guided_hunt);
}

#[test]
fn pheromone_hunt_evidence_expires_and_cannot_credit_a_different_victim() {
    let mut world = hunting_world(true, true);
    world.config.ecology.refuge_fraction = 1.0;
    world.step();
    assert!(world.agents[0].pheromone_guidance.is_some());
    assert!(!world.first_pheromone_guided_hunt);
    world.config.ecology.refuge_fraction = 0.0;
    let mut different = world.clone();
    different.agents[0]
        .pheromone_guidance
        .as_mut()
        .unwrap()
        .victim = 99;
    different.step();
    assert!(different.first_predation);
    assert!(!different.first_pheromone_guided_hunt);
    let mut stale = world.clone();
    stale.tick = 12; // No new decision for either agent this tick.
    stale.step();
    assert!(stale.first_predation);
    assert!(!stale.first_pheromone_guided_hunt);
    let mut resumed: World = postcard::from_bytes(&postcard::to_allocvec(&world).unwrap()).unwrap();
    world.step();
    resumed.step();
    assert!(world.first_pheromone_guided_hunt);
    assert_eq!(world.state_hash(), resumed.state_hash());
}
