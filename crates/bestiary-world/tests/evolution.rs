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
#[ignore = "five-seed million-tick evolutionary acceptance; invoked by make longrun"]
fn five_seed_end_of_run_trait_variance_and_archive_coverage_are_increasing() {
    let mut failures = vec![];
    for seed in 0..5 {
        let c = Config::parse(include_str!("../../../config/demo.ron")).unwrap();
        let mut a = World::new(seed, c, 24).unwrap();
        let mut baseline = None;
        for _ in 0..1_000_000 {
            a.step();
            if a.tick % 1000 == 0 {
                assert!(a.conserved(), "seed={seed} tick={}", a.tick);
                assert!(
                    !a.agents.is_empty(),
                    "population collapse seed={seed} tick={}",
                    a.tick
                );
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
        if after.coverage <= before.coverage || variance(&after) <= variance(&before) {
            failures.push(seed);
        }
    }
    assert!(
        failures.is_empty(),
        "stagnation acceptance failed for seeds {failures:?}"
    );
}
