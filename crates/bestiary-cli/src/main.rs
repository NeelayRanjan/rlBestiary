mod replay_export;
use bestiary_archive::store::Store;
use bestiary_core::genome::{Genome, Operator};
use bestiary_daemon::{load_snapshot, request, Request, Response, Result};
use bestiary_sim::{combine::evaluate, config::Config};
use bestiary_world::{hex, World};
use std::path::Path;
fn value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|x| x == flag)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}
fn number<T: std::str::FromStr>(args: &[String], flag: &str, default: T) -> Result<T>
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    Ok(value(args, flag)
        .map(str::parse)
        .transpose()?
        .unwrap_or(default))
}
fn arg(args: &[String], index: usize) -> Result<&str> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| "missing argument; run bestiary --help".into())
}
fn config(args: &[String]) -> Result<Config> {
    if let Some(path) = value(args, "--config") {
        Ok(Config::parse(&std::fs::read_to_string(path)?).map_err(|e| format!("config: {e}"))?)
    } else {
        Ok(Config::bundled())
    }
}
fn genome(input: &str, c: &Config) -> Result<Genome> {
    if Path::new(input).is_file() {
        let bytes = std::fs::read(input)?;
        if let Ok(text) = std::str::from_utf8(&bytes) {
            if let Ok(g) = Genome::from_share(text.trim(), &c.genome) {
                return Ok(g);
            }
        }
        let g: Genome = postcard::from_bytes(&bytes)?;
        g.validate(&c.genome)?;
        Ok(g)
    } else if let Some((_, g)) = bestiary_core::genome::zoo(&c.genome)
        .into_iter()
        .find(|(name, _)| *name == input)
    {
        Ok(g)
    } else {
        Ok(Genome::from_share(input, &c.genome)?)
    }
}
fn print_json(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
fn run(args: &[String]) -> Result<()> {
    if args.is_empty() || args[0] == "--help" {
        println!("bestiary world new --seed S --preset temperate|islands|harsh --out WORLD.db\nbestiary world run WORLD.db [--until TICKS] [--snapshot-every N] [--speed max|N]\nbestiary world attach|status WORLD.db\nbestiary combine GENOME_OR_PRESET [--replay OUT.rpl] [--json]\nbestiary archive ls WORLD.db [--export DIR]\nbestiary phylo WORLD.db [--format dot|json]\nbestiary replay FILE.rpl | ID [--world WORLD.db] [--export NEW_DIRECTORY]\nbestiary genome mutate|cross|validate|show GENOME [SECOND] [--seed S]\nbestiary bench [--report]\nbestiary train GENOME_OR_PRESET --task sprint|swim|turn|slope|rough|strike [--generations N] [--population P] [--seed S] [--body-mutation] [--training PATH] [--out FILE] [--replay FILE]\nAll commands support --json. Genome inputs are share strings, binary files, or zoo preset names.");
        return Ok(());
    }
    let json = args.iter().any(|s| s == "--json");
    match args[0].as_str() {
        "world" => match arg(args, 1)? {
            "new" => {
                let mut c = config(args)?;
                match value(args, "--preset").unwrap_or("temperate") {
                    "temperate" => {}
                    "islands" => {
                        c.world.water_level = 0.1;
                        c.world.terrain_frequency = 2.0;
                    }
                    "harsh" => {
                        c.world.solar_per_cell /= 2;
                        c.world.season_amplitude = 0.7;
                    }
                    other => return Err(format!("unknown scenario {other}").into()),
                }
                let path = Path::new(value(args, "--out").ok_or("--out is required")?);
                let world = World::new(
                    number(args, "--seed", 42u64)?,
                    c,
                    number(args, "--population", 64usize)?,
                )?;
                let mut store = Store::create(path)?;
                store.save(&world)?;
                store.events(world.tick, &world.ecological_events)?;
                if json {
                    print_json(&bestiary_daemon::status(&world, false))?;
                } else {
                    println!(
                        "Created {} with {} creatures",
                        path.display(),
                        world.agents.len()
                    );
                }
            }
            "run" => {
                let path = Path::new(arg(args, 2)?);
                let until = value(args, "--until").map(str::parse).transpose()?;
                let speed = match value(args, "--speed").unwrap_or("max") {
                    "max" => None,
                    v => Some(v.parse()?),
                };
                bestiary_daemon::run(
                    path,
                    until,
                    number(args, "--snapshot-every", 100u64)?,
                    speed,
                )?;
                if json {
                    print_json(&bestiary_daemon::status(&load_snapshot(path)?, false))?;
                }
            }
            "attach" => {
                let endpoint = bestiary_daemon::endpoint(Path::new(arg(args, 2)?))?;
                if json {
                    print_json(&serde_json::json!({"endpoint":endpoint}))?;
                } else {
                    println!("{endpoint}");
                }
            }
            "status" => {
                let path = Path::new(arg(args, 2)?);
                let store = Store::open(path, true)?;
                let status = match request(path, Request::Status) {
                    Ok(Response::Status(status)) => status,
                    Ok(Response::Error(error)) => return Err(error.into()),
                    Ok(_) => return Err("unexpected daemon status response".into()),
                    Err(_) => bestiary_daemon::status(&store.load()?, false),
                };
                if json {
                    print_json(&status)?;
                } else {
                    println!(
                        "tick={} population={} species={} paused={} hash={}",
                        status.tick,
                        status.population,
                        status.species,
                        status.paused,
                        status.state_hash
                    );
                }
            }
            other => return Err(format!("unknown world command {other}").into()),
        },
        "combine" => {
            let c = config(args)?;
            let g = genome(arg(args, 1)?, &c)?;
            let result = evaluate(&g, &c)?;
            if let Some(path) = value(args, "--replay") {
                let replay = result
                    .trials
                    .iter()
                    .find_map(|t| t.replay.as_ref())
                    .ok_or("no replay: stand gate failed")?;
                std::fs::write(path, replay.encode()?)?;
            }
            if json {
                print_json(
                    &serde_json::json!({"genome":hex(&result.genome_hash),"traits":result.traits,"viability":result.viability,"exploit_suspect":result.exploit_suspect}),
                )?;
            } else {
                println!(
                    "genome={} viability={:.5} speed={:.4} mass={:.4} parts={} exploit={}",
                    hex(&result.genome_hash),
                    result.viability,
                    result.traits.speed,
                    result.traits.mass,
                    result.traits.parts,
                    result.exploit_suspect
                );
            }
        }
        "train" => {
            use bestiary_sim::training::{Task, Training, TrainingSettings};
            let c = config(args)?;
            let source = genome(arg(args, 1)?, &c)?;
            let task = value(args, "--task")
                .and_then(Task::parse)
                .ok_or("--task must be sprint|swim|turn|slope|rough|strike")?;
            let mut settings = match value(args, "--training") {
                Some(path) => TrainingSettings::parse(&std::fs::read_to_string(path)?)
                    .map_err(|e| format!("training: {e}"))?,
                None => TrainingSettings::bundled(),
            };
            settings.population = number(args, "--population", settings.population)?;
            settings.generations = number(args, "--generations", settings.generations)?;
            settings.body_mutation |= args.iter().any(|a| a == "--body-mutation");
            settings.validate()?;
            let generations = settings.generations;
            let seed = number(args, "--seed", 42u64)?;
            let mut training = Training::new(source, task, settings, c, seed)?;
            let mut last = None;
            for _ in 0..generations {
                let r = training.next_generation(true)?;
                let champion = hex(&r.champion.hash());
                if json {
                    println!(
                        "{}",
                        serde_json::json!({"generation": r.generation, "best": r.best, "median": r.median, "worst": r.worst, "champion": champion})
                    );
                } else {
                    println!(
                        "generation={} best={:.4} median={:.4} worst={:.4} champion={champion}",
                        r.generation, r.best, r.median, r.worst
                    );
                }
                last = Some(r);
            }
            let last = last.ok_or("no generations run")?;
            if let Some(path) = value(args, "--out") {
                std::fs::write(path, last.champion.to_share())?;
            }
            if let Some(path) = value(args, "--replay") {
                std::fs::write(path, last.champion_replay.encode()?)?;
            }
        }
        "genome" => {
            let c = config(args)?;
            let g = genome(arg(args, 2)?, &c)?;
            match arg(args, 1)? {
                "validate" => {
                    let body = g.expand(&c.genome)?;
                    print_json(
                        &serde_json::json!({"valid":true,"parts":body.parts.len(),"mass":body.mass()}),
                    )?;
                }
                "show" => print_json(&g)?,
                "mutate" => {
                    let op = number(args, "--operator", 0usize)?;
                    let op = *Operator::ALL.get(op).ok_or("operator must be 0..22")?;
                    let (g, record) = g.mutate(op, number(args, "--seed", 42u64)?, 0, &c.genome);
                    if json {
                        print_json(&serde_json::json!({"genome":g.to_share(),"mutation":record}))?;
                    } else {
                        println!("{}", g.to_share());
                    }
                }
                "cross" => {
                    let other = genome(arg(args, 3)?, &c)?;
                    let child = g.crossover(&other, number(args, "--seed", 42u64)?, &c.genome);
                    if json {
                        print_json(&serde_json::json!({"genome":child.to_share()}))?;
                    } else {
                        println!("{}", child.to_share());
                    }
                }
                other => return Err(format!("unknown genome command {other}").into()),
            }
        }
        "phylo" => {
            let world = load_snapshot(Path::new(arg(args, 1)?))?;
            if value(args, "--format") == Some("dot") && !json {
                println!("digraph phylogeny {{");
                for s in world.registry.species.values() {
                    println!("  s{} [label=\"{}\"]", s.id, s.name);
                    if let Some(parent) = s.parent {
                        println!("  s{parent} -> s{}", s.id);
                    }
                }
                println!("}}");
            } else {
                print_json(&world.registry.species)?;
            }
        }
        "archive" => {
            if arg(args, 1)? != "ls" {
                return Err("use archive ls WORLD.db".into());
            }
            let world = load_snapshot(Path::new(arg(args, 2)?))?;
            let mut rows: Vec<_> = world.archive.cells.iter().collect();
            if value(args, "--sort") == Some("viability") {
                rows.sort_by(|a, b| b.1.viability.total_cmp(&a.1.viability).then(a.0.cmp(b.0)));
            }
            if let Some(directory) = value(args, "--export") {
                std::fs::create_dir_all(directory)?;
                let store = Store::open(Path::new(arg(args, 2)?), true)?;
                for (_, elite) in &rows {
                    let genome = &world.genomes[&elite.genome].genome;
                    std::fs::write(
                        Path::new(directory).join(format!("{}.genome", hex(&elite.genome))),
                        postcard::to_allocvec(genome)?,
                    )?;
                    std::fs::write(
                        Path::new(directory).join(format!("{}.rpl", hex(&elite.genome))),
                        store.replay(&elite.replay_id)?,
                    )?;
                }
                std::fs::write(
                    Path::new(directory).join("archive.json"),
                    serde_json::to_vec_pretty(&rows)?,
                )?;
            }
            print_json(
                &serde_json::json!({"coverage":world.archive.coverage(&world.config.archive),
                "total_cells":world.config.archive.total_cells(),"cells":rows}),
            )?;
        }
        "replay" => {
            let id = arg(args, 1)?;
            let output = value(args, "--export").filter(|value| !value.starts_with("--"));
            if args.iter().any(|arg| arg == "--export") && output.is_none() {
                return Err("--export requires a fresh output directory".into());
            }
            print_json(&replay_export::run(
                id,
                value(args, "--world").map(Path::new),
                output.map(Path::new),
            )?)?;
        }
        "bench" => {
            let c = config(args)?;
            let g = bestiary_core::genome::benchmark_genome(&c.genome);
            let start = std::time::Instant::now();
            let result = evaluate(&g, &c)?;
            let seconds = start.elapsed().as_secs_f64();
            print_json(
                &serde_json::json!({"combine_seconds":seconds,"parts":result.traits.parts,"passes":seconds<2.0}),
            )?;
            if seconds >= 2.0 {
                return Err("combine performance gate failed".into());
            }
        }
        other => return Err(format!("unknown command {other}; use --help").into()),
    }
    Ok(())
}
fn main() {
    if let Err(e) = run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        eprintln!("bestiary: {e}");
        std::process::exit(1);
    }
}
