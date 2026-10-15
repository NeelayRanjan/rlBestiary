//! Mutational reach of the demo founder: apply random operator chains, combine every
//! mutant in parallel, and report which archive cells and descriptor bins they reach.
//! `cargo run --release -p bestiary-world --example archive_probe -- CONFIG MUTANTS MAX_CHAIN`
use bestiary_archive::grid::{cell, descriptors, Archive, AXES};
use bestiary_core::genome::{Genome, Operator};
use bestiary_sim::{combine::evaluate, config::Config};
use bestiary_world::World;
use rand::Rng;
use rayon::prelude::*;
use std::collections::BTreeMap;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).map_or("config/demo.ron", String::as_str);
    let mutants: u64 = args.get(2).map_or(400, |a| a.parse().unwrap());
    let max_chain: usize = args.get(3).map_or(4, |a| a.parse().unwrap());
    let config = Config::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    let world = World::new(42, config.clone(), 1).unwrap();
    let founder: Genome = world.genomes.values().next().unwrap().genome.clone();
    let jobs: Vec<(Vec<&'static str>, Genome)> = (0..mutants)
        .map(|n| {
            let mut rng = bestiary_core::rng(42, 0x50524f42, n, 0);
            let mut g = founder.clone();
            let mut ops = vec![];
            for step in 0..rng.gen_range(1..=max_chain) {
                let op = Operator::ALL[rng.gen_range(0..Operator::ALL.len())];
                g = g.mutate(op, n, step as u64, &config.genome).0;
                ops.push(op.name());
            }
            (ops, g)
        })
        .collect();
    let results: Vec<_> = jobs
        .par_iter()
        .map(|(ops, g)| (ops.clone(), evaluate(g, &config)))
        .collect();
    let mut archive = Archive::default();
    let mut bins = vec![BTreeMap::<u8, usize>::new(); AXES.len()];
    let mut viable = 0;
    let mut per_op: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for (ops, result) in &results {
        let Ok(e) = result else { continue };
        let ok = !e.exploit_suspect && e.viability > 0.0;
        for op in ops {
            let entry = per_op.entry(op).or_default();
            entry.0 += 1;
            entry.1 += usize::from(ok);
        }
        if !ok {
            continue;
        }
        viable += 1;
        let key = cell(&e.traits, &config.archive);
        for (axis, bin) in key.iter().enumerate() {
            *bins[axis].entry(*bin).or_default() += 1;
        }
        archive.insert(e, 0, &config.archive);
    }
    let fe = evaluate(&founder, &config).unwrap();
    println!("founder descriptors {:?}", descriptors(&fe.traits));
    println!("founder cell {:?}", cell(&fe.traits, &config.archive));
    println!(
        "mutants={mutants} viable={viable} cells={} coverage={:.4}",
        archive.cells.len(),
        archive.coverage(&config.archive)
    );
    for (axis, name) in AXES.iter().enumerate() {
        println!("{name:>15}: {:?}", bins[axis]);
    }
    for (op, (n, ok)) in per_op {
        println!("{op:>22}: {ok}/{n} viable");
    }
    for key in archive.cells.keys() {
        println!("cell {key:?}");
    }
}
