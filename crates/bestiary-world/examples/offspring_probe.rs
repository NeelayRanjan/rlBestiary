//! Times one mating epoch with serial and parallel offspring combines.
//! `cargo run --release -p bestiary-world --example offspring_probe -- FOUNDERS RATE`
use bestiary_core::genome::Genome;
use bestiary_sim::config::Config;
use bestiary_world::{forager_brain, World};
use std::time::Instant;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let founders: usize = args.get(1).map_or(24, |a| a.parse().unwrap());
    let rate: f32 = args.get(2).map_or(0.3, |a| a.parse().unwrap());
    let mut config = Config::bundled();
    config.world.grid = 4;
    config.world.initial_energy = 500_000_000;
    config.world.decision_interval = 10_000;
    config.world.population_limit = founders * 4;
    config.reproduction.epoch = 1;
    config.reproduction.mutation_probability = 1.0;
    config.reproduction.compatibility_threshold = 1.0;
    let mut genome = Genome::starter(&config.genome);
    genome.ethology = forager_brain();
    genome.life.maturity = 2;
    genome.mutation_rates.fill(rate);
    let mut base = World::with_founder(42, config, founders, genome).unwrap();
    for cell in &mut base.cells {
        cell.height = 1.0;
    }
    base.tick = 100;
    for agent in &mut base.agents {
        agent.age = 3;
        agent.outputs = vec![0.0; 13];
        agent.outputs[7] = 1.0;
    }
    for serial in [true, false] {
        let mut w = base.clone();
        w.serial_combines = serial;
        let genomes = w.genomes.len();
        let start = Instant::now();
        w.step();
        println!(
            "serial={serial} evaluated={} births={} seconds={:.2} hash={}",
            w.genomes.len() - genomes,
            w.agents.len() - founders,
            start.elapsed().as_secs_f64(),
            bestiary_core::hex(&w.state_hash())
        );
    }
}
