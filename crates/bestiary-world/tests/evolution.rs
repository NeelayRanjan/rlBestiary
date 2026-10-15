mod acceptance;
use bestiary_archive::store::{Persistable, TimeSample};
use bestiary_sim::config::Config;
use bestiary_world::World;
fn variance(sample: &TimeSample) -> f32 {
    sample
        .trait_variance
        .iter()
        .zip(&sample.mean_traits)
        .map(|(v, m)| v / (1.0 + m * m))
        .sum()
}
#[test]
#[ignore = "five-seed million-tick evolutionary acceptance; make evolution-longrun"]
fn stagnation_guard_five_seed_end_of_run_trait_variance_and_archive_coverage_are_increasing() {
    let mut failures = vec![];
    for seed in acceptance::seeds() {
        let c = Config::parse(include_str!("../../../config/demo.ron")).unwrap();
        let mut a = World::new(seed, c, 24).unwrap();
        let mut baseline = None;
        let mut first_collapse = None;
        for _ in 0..1_000_000 {
            a.step();
            if a.tick % 1000 == 0 {
                assert!(a.conserved(), "seed={seed} tick={}", a.tick);
                if a.agents.is_empty() && first_collapse.is_none() {
                    first_collapse = Some(a.tick);
                }
                assert!(a.agents.len() <= a.config.world.population_limit);
                for g in a.genomes.values_mut() {
                    g.replays.clear();
                }
            }
            if a.tick == 750_000 {
                baseline = Some(a.sample());
            }
            if a.tick % 100_000 == 0 {
                println!(
                    "seed={seed} tick={} population={} coverage={:.4}",
                    a.tick,
                    a.agents.len(),
                    a.archive.coverage(&a.config.archive)
                );
            }
        }
        let before = baseline.unwrap();
        let after = a.sample();
        println!(
            "seed={seed} tail coverage {} -> {}, trait variance {} -> {}",
            before.coverage,
            after.coverage,
            variance(&before),
            variance(&after)
        );
        println!("seed={seed} first_collapse={first_collapse:?}");
        if first_collapse.is_some()
            || after.coverage <= before.coverage
            || variance(&after) <= variance(&before)
        {
            failures.push(seed);
        }
    }
    assert!(
        failures.is_empty(),
        "stagnation acceptance failed for seeds {failures:?}"
    );
}
