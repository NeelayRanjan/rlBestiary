use bestiary_sim::config::Config;
use bestiary_world::World;
use std::time::Instant;
fn main() {
    let mut config = Config::bundled();
    config.world.starvation_damage = 0.0;
    let mut world = World::new(42, config, 5000).unwrap();
    for _ in 0..20 {
        world.step();
    }
    let start = Instant::now();
    for _ in 0..200 {
        world.step();
    }
    let elapsed = start.elapsed().as_secs_f64();
    let ticks_per_minute = 200.0 * 60.0 / elapsed;
    assert_eq!(world.agents.len(), 5000);
    assert!(world.conserved());
    println!("5000 creatures, single-thread world stepping: {ticks_per_minute:.1} ticks/minute");
    assert!(ticks_per_minute >= 10000.0, "world throughput gate failed");
}
