use bestiary_sim::{config::Config, physics::run_fixture, PHYSICS_DT};
use criterion::{black_box, Criterion, Throughput};
use std::time::{Duration, Instant};

fn evaluation() {
    let config = Config::bundled().physics;
    let ticks = (config.benchmark_trial_seconds / PHYSICS_DT).round() as u32;
    for _ in 0..9 {
        black_box(run_fixture(config.clone(), ticks, true));
    }
}
fn main() {
    let config = Config::bundled().physics;
    evaluation(); // warm up code/data without counting the warm-up
    let mut worst = 0.0f64;
    for _ in 0..3 {
        let start = Instant::now();
        evaluation();
        worst = worst.max(start.elapsed().as_secs_f64());
    }
    let simulated = f64::from(config.benchmark_trial_seconds) * 9.0;
    println!("M1 representative physics gate: 12 parts, 9 workloads, {simulated} simulated creature-seconds");
    println!(
        "worst_seconds={worst:.6} creature_seconds_per_wall_second={:.3}",
        simulated / worst
    );
    assert!(
        worst < 2.0,
        "M1 physics throughput gate failed: {worst}s >= 2s"
    );
    let mut c = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    let mut group = c.benchmark_group("physics_12_parts_nine_workloads");
    group.throughput(Throughput::Elements((simulated * 240.0) as u64));
    group.bench_function("muscles_energy_audit_and_trajectory", |b| {
        b.iter(evaluation)
    });
    group.finish();
    c.final_summary();
}
