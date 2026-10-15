//! Command-line replay exports share the interactive camera clip pipeline.
use super::*;
use bestiary_archive::store::Store;
use std::path::Path;

#[derive(Resource)]
pub(crate) struct CameraOnly;

fn argument<'a>(args: &'a [String], flag: &str) -> Result<&'a str, String> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .filter(|value| !value.starts_with("--"))
        .map(String::as_str)
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn parse_hash(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 || !value.is_ascii() {
        return Err("--genome requires a 64-character hexadecimal hash".into());
    }
    let mut hash = [0; 32];
    for (index, byte) in hash.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| "--genome contains invalid hexadecimal digits")?;
    }
    Ok(hash)
}

pub fn configure(args: &[String], view: &mut View) -> Result<bool, String> {
    if !args.iter().any(|arg| arg == "--export-replay") {
        return Ok(false);
    }
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--smoke" | "--benchmark"))
    {
        return Err("--export-replay cannot be combined with --smoke or --benchmark".into());
    }
    let source = argument(args, "--export-replay")?;
    let directory = PathBuf::from(argument(args, "--export")?);
    let hash = parse_hash(argument(args, "--genome")?)?;
    let genome = view
        .snapshot
        .world
        .genomes
        .get(&hash)
        .ok_or("export genome is absent from this world")?;
    let store = Store::open(Path::new(&args[0]), true).map_err(|error| error.to_string())?;
    let bytes = if Path::new(source).is_file() {
        std::fs::read(source).map_err(|error| error.to_string())?
    } else {
        if !(0..2).any(|index| source == format!("{}-{index}", bestiary_core::hex(&hash))) {
            return Err("replay ID does not belong to the specified genome".into());
        }
        match bestiary_daemon::request(
            Path::new(&args[0]),
            bestiary_daemon::Request::Replay(source.into()),
        ) {
            Ok(bestiary_daemon::Response::Replay(bytes)) => bytes,
            _ => store.replay(source).map_err(|error| error.to_string())?,
        }
    };
    let belongs_to_genome = genome.replays.iter().any(|recorded| recorded == &bytes)
        || (0..2).any(|index| {
            store
                .replay(&format!("{}-{index}", bestiary_core::hex(&hash)))
                .is_ok_and(|recorded| recorded == bytes)
        });
    if !belongs_to_genome {
        return Err("replay is not recorded for the specified genome in this world".into());
    }
    let replay = Replay::decode(&bytes)?;
    if argument(args, "--trial")?.to_lowercase() != format!("{:?}", replay.kind).to_lowercase() {
        return Err("requested trial does not match the recorded replay".into());
    }
    let schedule = playback::ClipSchedule::recorded(replay.frames.len(), replay.dt, 30)?;
    view.tab = Tab::Inspector;
    view.genome = hash;
    view.selected = data::matching_creature(&view.snapshot.world, view.selected, hash);
    view.trial = replay.kind;
    view.live_preview = false;
    view.playing = false;
    view.play_time = 0.0;
    view.replay_frame = 0;
    view.radius = 5.0;
    view.replays.insert((hash, replay.kind), replay);
    start(view, directory, schedule, true)?;
    Ok(true)
}

pub fn start(
    view: &mut View,
    directory: PathBuf,
    schedule: playback::ClipSchedule,
    exit_after: bool,
) -> Result<(), String> {
    let parent = directory
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    std::fs::create_dir(&directory).map_err(|error| {
        format!(
            "cannot create fresh export directory {}: {error}",
            directory.display()
        )
    })?;
    let mut metadata = schedule.metadata(
        view.snapshot.world.tick,
        &bestiary_core::hex(&view.body_hash()),
        &format!("{:?}", view.trial),
    );
    metadata["complete"] = false.into();
    metadata["world_state_hash"] = bestiary_core::hex(&view.snapshot.world.state_hash()).into();
    metadata["camera_only"] = true.into();
    metadata["loop_replay"] = (!exit_after).into();
    std::fs::write(directory.join("ffmpeg.txt"),
        "Camera frames only; interface overlays are omitted.\nRun in this directory:\nffmpeg -framerate 30 -i frame-%05d.png -c:v libx264 -pix_fmt yuv420p clip.mp4\n")
        .and_then(|_| std::fs::write(directory.join("clip.json"), metadata.to_string()))
        .map_err(|error| error.to_string())?;
    view.clip = Some(Clip {
        dir: directory,
        schedule,
        resume_playing: view.playing,
        exit_after,
        first_frame_at: if exit_after { 90 } else { view.frame },
        metadata,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_entry_rejects_missing_values_and_invalid_genome_hashes() {
        assert!(argument(&["--export".into()], "--export").is_err());
        assert!(argument(&["--export".into(), "--headless".into()], "--export").is_err());
        assert!(parse_hash("bad").is_err());
        assert!(parse_hash(&"é".repeat(32)).is_err());
        assert!(parse_hash(&"g".repeat(64)).is_err());
        assert_eq!(parse_hash(&"0a".repeat(32)).unwrap(), [10; 32]);
    }
}
