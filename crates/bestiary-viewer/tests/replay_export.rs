use bestiary_archive::store::Store;
use bestiary_sim::{
    combine::{Replay, TrialKind},
    config::Config,
};
use bestiary_world::World;
use bevy::{
    image::{CompressedImageFormats, ImageSampler, ImageType},
    prelude::Image,
    render::render_asset::RenderAssetUsages,
};
use std::{
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Renderer(Child);
impl Drop for Renderer {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn pixels(path: &Path) -> Vec<u8> {
    let image = Image::from_buffer(
        &std::fs::read(path).unwrap(),
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::default(),
        RenderAssetUsages::default(),
    )
    .unwrap()
    .try_into_dynamic()
    .unwrap()
    .to_rgb8();
    assert_eq!((image.width(), image.height()), (2880, 1920));
    let colors: std::collections::BTreeSet<_> = image.pixels().step_by(97).map(|p| p.0).collect();
    assert!(colors.len() > 32, "empty camera export");
    image.into_raw()
}

#[test]
fn export_rejects_mismatched_id_genome_and_conflicting_render_modes_before_gpu_startup() {
    let directory =
        std::env::temp_dir().join(format!("bestiary-export-invalid-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("world.db");
    let mut config = Config::bundled();
    config.world.grid = 8;
    let mut world = World::new(42, config, 2).unwrap();
    let hash = *world.genomes.keys().next().unwrap();
    let mut neutral = world.genomes[&hash].clone();
    neutral.genome.bones[0].color[0] += 0.01;
    let other = neutral.genome.hash();
    assert_ne!(hash, other);
    assert_eq!(world.genomes[&hash].replays, neutral.replays);
    world.genomes.insert(other, neutral);
    Store::create(&path).unwrap().save(&world).unwrap();
    for (selected, extra, error) in [
        (other, None, "ID does not belong"),
        (hash, Some("--smoke"), "cannot be combined"),
        (hash, Some("--benchmark"), "cannot be combined"),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_bestiary-viewer"));
        command
            .arg(&path)
            .args([
                "--export-replay",
                &format!("{}-0", bestiary_core::hex(&hash)),
                "--genome",
                &bestiary_core::hex(&selected),
                "--trial",
                "Sprint",
                "--export",
            ])
            .arg(directory.join("out"));
        if let Some(flag) = extra {
            command.arg(flag).arg(directory.join("smoke.png"));
        }
        let result = command.output().unwrap();
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains(error));
        assert!(!directory.join("out").exists());
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
#[ignore = "requires GPU and sibling bestiary CLI; build CLI, then cargo test -p bestiary-viewer --test replay_export -- --ignored --nocapture"]
fn cli_recorded_replay_png_export_contains_motion_and_preserves_the_world() {
    check_export(false);
}

#[test]
#[ignore = "requires GPU and sibling bestiary CLI; build CLI, then cargo test -p bestiary-viewer --test replay_export -- --ignored --nocapture"]
fn cli_recorded_file_png_export_uses_world_anatomy_and_preserves_the_world() {
    check_export(true);
}

fn check_export(from_file: bool) {
    let directory = std::env::temp_dir().join(format!(
        "bestiary-gpu-export-{from_file}-{}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("world.db");
    let output = directory.join("png sequence");
    let mut config = Config::bundled();
    config.world.grid = 8;
    let world = World::new(42, config, 2).unwrap();
    let hash = *world.genomes.keys().next().unwrap();
    let (index, replay) = world.genomes[&hash]
        .replays
        .iter()
        .enumerate()
        .find_map(|(i, bytes)| {
            let replay = Replay::decode(bytes).unwrap();
            (replay.kind == TrialKind::Strike).then_some((i, replay))
        })
        .unwrap();
    Store::create(&path).unwrap().save(&world).unwrap();
    let source = if from_file {
        let file = directory.join("recorded strike.rpl");
        std::fs::write(&file, &world.genomes[&hash].replays[index]).unwrap();
        file.to_str().unwrap().to_owned()
    } else {
        format!("{}-{index}", bestiary_core::hex(&hash))
    };
    let before = std::fs::read(&path).unwrap();
    let log_path = directory.join("renderer.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let cli = Path::new(env!("CARGO_BIN_EXE_bestiary-viewer"))
        .with_file_name(format!("bestiary{}", std::env::consts::EXE_SUFFIX));
    assert!(
        cli.is_file(),
        "build bestiary-cli before running this GPU integration test"
    );
    let mut renderer = Renderer(
        Command::new(cli)
            .args(["replay", &source, "--world"])
            .arg(&path)
            .arg("--export")
            .arg(&output)
            .arg("--json")
            .stdout(Stdio::from(log.try_clone().unwrap()))
            .stderr(Stdio::from(log))
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if let Some(status) = renderer.0.try_wait().unwrap() {
            assert!(
                status.success(),
                "{status}: {}",
                std::fs::read_to_string(&log_path).unwrap()
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "export timeout: {}",
            std::fs::read_to_string(&log_path).unwrap()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.join("clip.json")).unwrap()).unwrap();
    assert_eq!(metadata["complete"], true);
    assert_eq!(metadata["camera_only"], true);
    assert_eq!(metadata["loop_replay"], false);
    assert_eq!(metadata["trial"], "Strike");
    assert_eq!(metadata["fps"], 30);
    let count = metadata["frames"].as_u64().unwrap() as usize;
    let final_time = (replay.frames.len() - 1) as f32 * replay.dt;
    assert!(metadata["last_frame_seconds"].as_f64().unwrap() >= f64::from(final_time));
    let pngs = std::fs::read_dir(&output)
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|extension| extension == "png")
        })
        .count();
    assert_eq!(pngs, count);
    let first = pixels(&output.join("frame-00000.png"));
    let last = pixels(&output.join(format!("frame-{:05}.png", count - 1)));
    assert_ne!(
        first, last,
        "recorded poses must animate rather than duplicate a static image"
    );
    assert!(std::fs::read_to_string(output.join("ffmpeg.txt"))
        .unwrap()
        .contains("-framerate 30"));
    assert_eq!(before, std::fs::read(&path).unwrap());
    let after: World = Store::open(&path, true).unwrap().load().unwrap();
    assert_eq!(world.state_hash(), after.state_hash());
    println!("exported {count} distinct-time 2880x1920 camera frames from {} recorded poses; world unchanged", replay.frames.len());
    std::fs::remove_dir_all(directory).unwrap();
}
