use bestiary_archive::store::Store;
use bestiary_daemon::{load_snapshot, request, Request, Response, Result};
use bestiary_sim::combine::Replay;
use bestiary_world::{hex, World};
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
};

fn source_bytes(source: &str, world: Option<&Path>) -> Result<Vec<u8>> {
    if Path::new(source).is_file() {
        return Ok(std::fs::read(source)?);
    }
    let path = world.ok_or("a replay ID requires --world WORLD.db")?;
    let store = Store::open(path, true)?;
    if let Ok(Response::Replay(bytes)) = request(path, Request::Replay(source.into())) {
        return Ok(bytes);
    }
    store.replay(source)
}

fn recorded_genome(source: &str, world: &World, store: &Store, bytes: &[u8]) -> Result<[u8; 32]> {
    let mut matches = world.genomes.iter().filter_map(|(hash, genome)| {
        if !Path::new(source).is_file()
            && !(0..2).any(|index| source == format!("{}-{index}", hex(hash)))
        {
            return None;
        }
        let matches = genome.replays.iter().any(|cached| cached == bytes)
            || (0..2).any(|index| {
                store
                    .replay(&format!("{}-{index}", hex(hash)))
                    .is_ok_and(|recorded| recorded == bytes)
            });
        matches.then_some(*hash)
    });
    let hash = matches
        .next()
        .ok_or("replay is not recorded in this world; its anatomy cannot be identified")?;
    if matches.next().is_some() {
        return Err("replay file matches more than one genome; export its recorded replay ID to select exact anatomy".into());
    }
    Ok(hash)
}

pub fn run(
    source: &str,
    world_path: Option<&Path>,
    output: Option<&Path>,
) -> Result<serde_json::Value> {
    if output.is_some() && world_path.is_none() {
        return Err(
            "PNG export requires --world WORLD.db to supply the recorded creature's anatomy".into(),
        );
    }
    let bytes = source_bytes(source, world_path)?;
    let replay = Replay::decode(&bytes)?;
    let mut report =
        serde_json::json!({"kind":replay.kind,"frames":replay.frames.len(),"dt":replay.dt});
    if let Some(output) = output {
        if replay.frames.is_empty() {
            return Err("cannot export a replay with no recorded frames".into());
        }
        if output.exists() {
            return Err("export directory already exists; choose a fresh directory".into());
        }
        let path = world_path.expect("checked world path");
        let world = load_snapshot(path)?;
        let store = Store::open(path, true)?;
        let genome = recorded_genome(source, &world, &store, &bytes)?;
        let viewer = std::env::current_exe()?
            .with_file_name(format!("bestiary-viewer{}", std::env::consts::EXE_SUFFIX));
        let status = Command::new(&viewer)
            .arg(path)
            .args(["--export-replay", source, "--genome", &hex(&genome), "--trial", &format!("{:?}", replay.kind)])
            .arg("--export")
            .arg(output)
            .arg("--headless")
            .stdout(Stdio::null())
            .status()
            .map_err(|error| format!("cannot launch sibling viewer {}: {error}; build or install bestiary-viewer beside bestiary", viewer.display()))?;
        if !status.success() {
            return Err(format!("replay renderer failed: {status}").into());
        }
        let metadata: serde_json::Value =
            serde_json::from_slice(&std::fs::read(output.join("clip.json"))?)?;
        let last_time = (replay.frames.len() - 1) as f32 * replay.dt;
        let expected = (f64::from(last_time) * 30.0).ceil() as u64 + 1;
        if metadata["complete"] != true
            || metadata["frames"] != expected
            || metadata["fps"] != 30
            || metadata["genome"] != hex(&genome)
            || metadata["trial"] != format!("{:?}", replay.kind)
        {
            return Err("renderer stopped before completing the PNG sequence".into());
        }
        for index in 0..expected {
            let path = output.join(format!("frame-{index:05}.png"));
            let mut signature = [0u8; 8];
            let valid = std::fs::File::open(&path)
                .and_then(|mut file| file.read_exact(&mut signature))
                .is_ok()
                && signature == *b"\x89PNG\r\n\x1a\n";
            if !valid {
                return Err(format!(
                    "renderer reported completion but PNG frame {} is missing, empty or invalid",
                    path.display()
                )
                .into());
            }
        }
        report["export"] = serde_json::json!({"directory":output,"frames":metadata["frames"],"fps":metadata["fps"]});
    }
    Ok(report)
}
