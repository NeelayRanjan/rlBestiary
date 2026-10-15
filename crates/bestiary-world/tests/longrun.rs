mod acceptance;
use bestiary_core::genome::Genome;
use bestiary_sim::config::Config;
use bestiary_world::{forager_brain, World};

fn founder(config: &Config) -> Genome {
    let mut g = Genome::starter(&config.genome);
    g.ethology = forager_brain();
    // M4 precedes reproduction. This fixture's life span exceeds the validation horizon.
    g.life.senescence = 2_000_000;
    g
}
#[test]
fn conservation_determinism_and_snapshot_resume() {
    let mut config = Config::bundled();
    config.world.grid = 8;
    config.reproduction.epoch = 2_000_000;
    let g = founder(&config);
    let mut a = World::with_founder(42, config, 8, g).unwrap();
    let mut b = a.clone();
    for _ in 0..1000 {
        a.step();
        b.step();
        assert!(a.conserved());
    }
    assert_eq!(a.state_hash(), b.state_hash());
    assert!(!a.agents.is_empty());
    let bytes = postcard::to_allocvec(&a).unwrap();
    let mut resumed: World = postcard::from_bytes(&bytes).unwrap();
    for _ in 0..100 {
        a.step();
        resumed.step();
    }
    assert_eq!(a.state_hash(), resumed.state_hash());
}
#[test]
#[ignore = "one-million-tick acceptance run; invoked by make longrun"]
fn five_seeds_one_million_ticks_conservation_stability_and_determinism() {
    for seed in acceptance::seeds() {
        let config = Config::parse(include_str!("../../../config/validation.ron")).unwrap();
        let mut a = World::new(seed, config.clone(), 24).unwrap();
        let mut b = World::new(seed, config, 24).unwrap();
        let mut first_collapse = None;
        for tick in 1..=1_000_000 {
            a.step();
            b.step();
            assert!(a.conserved() && b.conserved(), "seed={seed}, tick={tick}");
            if a.agents.is_empty() && first_collapse.is_none() {
                first_collapse = Some(tick);
            }
            assert!(a.agents.len() <= a.config.world.population_limit);
            if tick % 1000 == 0 {
                for w in [&mut a, &mut b] {
                    for g in w.genomes.values_mut() {
                        g.replays.clear();
                    }
                }
            }
            if [1000, 100_000, 1_000_000].contains(&tick) {
                assert_eq!(
                    a.state_hash(),
                    b.state_hash(),
                    "determinism seed={seed} tick={tick}"
                );
                println!(
                    "seed={seed} tick={tick} hash={} population={} conserved=true",
                    bestiary_core::hex(&a.state_hash()),
                    a.agents.len()
                );
            }
        }
        assert!(
            first_collapse.is_none(),
            "collapse seed={seed} tick={first_collapse:?}"
        );
        let births = a.history.values().filter(|h| !h.parents.is_empty()).count();
        let deaths = a.history.values().filter(|h| h.died.is_some()).count();
        assert!(
            births > 0 && deaths > 0,
            "real births and deaths seed={seed}"
        );
        assert!(
            a.history
                .values()
                .filter(|h| h.born == 0)
                .all(|h| h.died.is_some()),
            "founders must turn over seed={seed}"
        );
        println!("seed={seed} ticks={} population={} births={births} deaths={deaths} exact_conservation=true", a.tick, a.agents.len());
    }
}
#[test]
fn death_and_decay_preserve_both_resources() {
    let mut config = Config::bundled();
    config.world.grid = 4;
    let mut w = World::new(7, config, 2).unwrap();
    w.agents[0].health = -1.0;
    for _ in 0..1000 {
        w.step();
        assert!(w.conserved());
    }
    assert!(w.agents.len() < 2);
    assert!(w
        .cells
        .iter()
        .all(|c| c.soil >= 0 && c.plant.energy >= 0 && c.plant.nutrients >= 0));
}
#[test]
fn world_config_rejects_invalid_ranges() {
    let mut c = Config::bundled().world;
    c.grid = 0;
    assert!(c.validate().is_err());
    c = Config::bundled().world;
    c.seconds_per_tick = f32::NAN;
    assert!(c.validate().is_err());
    c = Config::bundled().world;
    c.decision_interval = 0;
    assert!(c.validate().is_err());
}
