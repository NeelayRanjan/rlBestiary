use bestiary_core::genome::benchmark_genome;
use bestiary_sim::{combine::evaluate, config::Config};
use criterion::{black_box, Criterion, Throughput};
use std::time::{Duration, Instant};
fn main() {
    let config = Config::bundled();
    let genome = benchmark_genome(&config.genome);
    assert_eq!(genome.expand(&config.genome).unwrap().parts.len(), 12);
    let evaluation = || {
        let result = evaluate(&genome, &config).expect("valid benchmark genome");
        assert_eq!(result.trials.len(), 9);
        assert!(result.trials.iter().all(|t| !t.skipped));
        black_box(result);
    };
    evaluation();
    let mut worst = 0.0f64;
    for _ in 0..3 {
        let start = Instant::now();
        evaluation();
        worst = worst.max(start.elapsed().as_secs_f64());
    }
    let simulated =
        f64::from(config.combine.trial_seconds) * (8 + config.combine.slope_angles.len()) as f64;
    println!("Full combine: 12 parts, nine named trials (slope sweep included), {simulated} creature-seconds");
    println!(
        "worst_seconds={worst:.6} creature_seconds_per_wall_second={:.3}",
        simulated / worst
    );
    assert!(
        worst < 2.0,
        "combine performance gate failed: {worst}s >= 2s"
    );
    let mut c = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    let mut group = c.benchmark_group("combine_12_parts_nine_trials");
    group.throughput(Throughput::Elements(9));
    group.bench_function("full_evaluation", |b| b.iter(evaluation));
    group.finish();
    c.final_summary();
}
