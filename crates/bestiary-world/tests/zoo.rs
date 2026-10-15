//! Designer presets are tested as the exact genomes a user releases, into the
//! documented seed-42 demo ecology with its ordinary resident grazers.
use bestiary_core::genome::{self, Genome};
use bestiary_sim::{
    combine::{evaluate, TrialKind},
    config::Config,
};
use bestiary_world::World;
use std::collections::BTreeSet;

fn round_trips_combines_and_survives_first_generation(name: &str) {
    let config = Config::parse(include_str!("../../../config/demo.ron")).unwrap();
    let genome = genome::zoo(&config.genome)
        .into_iter()
        .find(|(candidate, _)| *candidate == name)
        .unwrap()
        .1;
    let binary = postcard::to_allocvec(&genome).unwrap();
    assert_eq!(postcard::from_bytes::<Genome>(&binary).unwrap(), genome);
    let shared = Genome::from_share(&genome.to_share(), &config.genome).unwrap();
    assert_eq!(shared, genome);
    assert!(genome.life.maturity > 0);
    assert!(genome.life.senescence > genome.life.maturity);
    let evaluation = evaluate(&shared, &config).unwrap();
    assert!(evaluation.viability > 0.0, "{name}: failed the stand gate");
    assert!(!evaluation.exploit_suspect, "{name}: energy audit failed");
    assert_eq!(evaluation.trials.len(), 9);
    assert!(evaluation
        .trials
        .iter()
        .any(|trial| trial.kind == TrialKind::Sprint && trial.replay.is_some()));

    // A flesh-eating preset needs the normal resident food web; a sterile
    // single-genome predator world would test starvation, not designer release.
    let mut world = World::new(42, config, 24).unwrap();
    let first_founder = world.next_id;
    // Normal compatible outcrossing is allowed: at least one original released
    // preset parent must appear on the child's actual two-parent birth record.
    // Choose the highest terrain patch so the initial location remains land
    // through the geographic cycle; release location is a normal designer input.
    // An interior offset avoids overlapping an existing resident at cell center.
    let cell = world
        .cells
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.height.total_cmp(&b.1.height))
        .unwrap()
        .0;
    let position = [
        ((cell % world.config.world.grid) as f32 + 0.6) * world.config.world.cell_metres,
        ((cell / world.config.world.grid) as f32 + 0.6) * world.config.world.cell_metres,
    ];
    world.release(shared, position, 2).unwrap();
    let released = world.next_id - first_founder;
    let founders = world
        .agents
        .iter()
        .filter(|agent| agent.genome == genome.hash())
        .map(|agent| agent.id)
        .collect::<BTreeSet<_>>();
    assert!(world.conserved());
    assert_eq!(world.genomes[&genome.hash()].traits, evaluation.traits);
    let deadline = genome.life.senescence + genome.life.maturity;
    let mut mature_child = None;
    for _ in 0..deadline {
        world.step();
        assert!(world.conserved(), "{name}: conservation at {}", world.tick);
        assert!(world.agents.len() <= world.config.world.population_limit);
        mature_child = world.agents.iter().find_map(|agent| {
            let genes = &world.genomes[&agent.genome].genome;
            (agent.parents.len() == 2
                && agent
                    .parents
                    .iter()
                    .any(|id| *id >= first_founder && founders.contains(id))
                && agent.age >= genes.life.maturity
                && agent.growth >= 0.8
                && agent.health > 0.0)
                .then_some((agent.id, agent.age))
        });
        if mature_child.is_some() || world.agents.is_empty() {
            break;
        }
    }
    let direct_births = world
        .history
        .values()
        .filter(|record| {
            record.parents.len() == 2
                && record
                    .parents
                    .iter()
                    .any(|id| *id >= first_founder && founders.contains(id))
        })
        .count();
    println!("preset={name:?} tick={} released={} direct_births={direct_births} mature_child={mature_child:?} population={} deaths={} conserved={}",
        world.tick, released, world.agents.len(), world.history.values().filter(|record| record.died.is_some()).count(), world.conserved());
    if mature_child.is_none() {
        for record in world.history.values().filter(|record| {
            founders.contains(&record.id)
                || record
                    .parents
                    .iter()
                    .any(|id| *id >= first_founder && founders.contains(id))
        }) {
            let genes = &world.genomes[&record.genome].genome;
            println!(
                "{name}: lineage {:?} maturity={} senescence={}",
                record, genes.life.maturity, genes.life.senescence
            );
            if let Some(agent) = world.agents.iter().find(|agent| agent.id == record.id) {
                println!(
                    "survivor age={} growth={} health={}",
                    agent.age, agent.growth, agent.health
                );
            }
        }
    }
    assert!(
        mature_child.is_some(),
        "{name}: no natural child of a released preset founder survived to maturity by tick {}",
        world.tick
    );
}

macro_rules! preset_acceptance {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            round_trips_combines_and_survives_first_generation($name);
        }
    };
}
preset_acceptance!(
    grazer_round_trips_combines_and_survives_first_generation,
    "grazer"
);
preset_acceptance!(
    sprinter_round_trips_combines_and_survives_first_generation,
    "sprinter"
);
preset_acceptance!(
    ambush_predator_round_trips_combines_and_survives_first_generation,
    "ambush predator"
);
preset_acceptance!(
    scavenger_round_trips_combines_and_survives_first_generation,
    "scavenger"
);
preset_acceptance!(
    burrower_round_trips_combines_and_survives_first_generation,
    "burrower"
);
preset_acceptance!(
    swimmer_round_trips_combines_and_survives_first_generation,
    "swimmer"
);
preset_acceptance!(
    serpent_round_trips_combines_and_survives_first_generation,
    "serpent"
);
preset_acceptance!(
    radial_drifter_round_trips_combines_and_survives_first_generation,
    "radial drifter"
);
