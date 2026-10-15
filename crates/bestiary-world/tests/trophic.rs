mod support;
use bestiary_archive::store::Persistable;
use bestiary_sim::config::Config;
use bestiary_world::{EcologyEvent, World};

fn counts(w: &World) -> (usize, usize) {
    (
        w.history.values().filter(|h| !h.parents.is_empty()).count(),
        w.history.values().filter(|h| h.died.is_some()).count(),
    )
}

#[test]
fn trophic_acceptance_fixture_has_physical_founders_and_conserved_resources() {
    let c = Config::parse(include_str!("../../../config/trophic.ron")).unwrap();
    let w = support::trophic(0, c).unwrap();
    assert!(w.conserved());
    assert_eq!(w.agents.len(), 44);
    assert!(w.genomes.values().all(|g| g.viability > 0.0 && !g.exploit));
}

#[test]
#[ignore = "million-tick trophic acceptance; make ecology-longrun"]
fn trophic_levels_all_persist_for_one_million_ticks_on_three_of_five_seeds() {
    let mut passed = 0;
    for seed in 0..5 {
        let c = Config::parse(include_str!("../../../config/trophic.ron")).unwrap();
        let mut w = support::trophic(seed, c).unwrap();
        let mut first_missing = None;
        let mut predation = 0;
        for _ in 0..1_000_000 {
            w.step();
            predation += w
                .ecological_events
                .iter()
                .filter(|e| matches!(e, EcologyEvent::Predation { .. }))
                .count();
            // Plants, plant consumers and flesh consumers define trophic levels.
            // Scavengers share the flesh tier; an omnivore diet bin is not a fourth level.
            let herbivores = w
                .agents
                .iter()
                .any(|a| w.genomes[&a.genome].genome.life.plant_efficiency >= 0.65);
            let carnivores = w
                .agents
                .iter()
                .any(|a| w.genomes[&a.genome].genome.life.plant_efficiency <= 0.35);
            if (!herbivores || !carnivores || !w.cells.iter().any(|c| c.plant.energy > 0))
                && first_missing.is_none()
            {
                first_missing = Some(w.tick);
            }
            if w.tick % 1000 == 0 {
                assert!(w.conserved(), "seed={seed} tick={}", w.tick);
                assert!(w.agents.len() <= w.config.world.population_limit);
                for g in w.genomes.values_mut() {
                    g.replays.clear();
                }
            }
            if w.tick % 100_000 == 0 {
                let (births, deaths) = counts(&w);
                println!("seed={seed} tick={} population={} births={births} deaths={deaths} first_missing={first_missing:?} predation={predation} conserved=true", w.tick, w.agents.len());
            }
        }
        let (births, deaths) = counts(&w);
        if first_missing.is_none() && births > 0 && deaths > 0 && predation > 0 {
            passed += 1;
        }
    }
    assert!(
        passed >= 3,
        "trophic persistence: {passed}/5 seeds, requires >=3"
    );
}

#[test]
#[ignore = "herbivore/plant evolutionary time series; make ecology-longrun"]
fn herbivore_only_toxic_plants_show_arms_race_in_trait_time_series() {
    let mut c = Config::parse(include_str!("../../../config/trophic.ron")).unwrap();
    c.world.population_limit = 64;
    let mut w = World::new(42, c, 24).unwrap();
    let initial = w.sample();
    let mut samples = vec![initial.clone()];
    for _ in 0..1_000_000 {
        w.step();
        if w.tick % 1000 == 0 {
            assert!(w.conserved());
            samples.push(w.sample());
            for g in w.genomes.values_mut() {
                g.replays.clear();
            }
        }
        if w.tick % 100_000 == 0 {
            let s = w.sample();
            println!(
                "arms_race tick={} population={} toxicity={} resistance={}",
                w.tick, s.population, s.plant_toxicity, s.herbivore_resistance
            );
        }
    }
    let final_sample = samples.last().unwrap();
    assert!(
        samples.iter().all(|s| s.population > 0),
        "herbivores persist"
    );
    assert!(counts(&w).0 > 0 && counts(&w).1 > 0, "real turnover");
    assert!(
        final_sample.plant_toxicity > initial.plant_toxicity + 1e-4,
        "plant defense increases"
    );
    assert!(
        final_sample.herbivore_resistance > initial.herbivore_resistance + 1e-4,
        "herbivore counter-defense increases"
    );
}
