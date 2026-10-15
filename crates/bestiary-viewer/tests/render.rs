use bestiary_archive::store::Store;
use bestiary_daemon::{request, Request, Response};
use bestiary_sim::config::Config;
use bestiary_world::World;
use bevy::{
    image::{CompressedImageFormats, ImageSampler, ImageType},
    prelude::Image,
    render::render_asset::RenderAssetUsages,
};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread::JoinHandle,
    time::{Duration, Instant},
};

struct TemporaryDirectory(PathBuf);

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Viewer(Child);

impl Drop for Viewer {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Daemon {
    path: PathBuf,
    thread: Option<JoinHandle<Result<(), String>>>,
}

impl Daemon {
    fn start(path: &Path) -> Self {
        let world_path = path.to_owned();
        let thread = std::thread::spawn(move || {
            bestiary_daemon::run(&world_path, None, 100, Some(1.0)).map_err(|e| e.to_string())
        });
        let mut daemon = Self {
            path: path.to_owned(),
            thread: Some(thread),
        };
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if matches!(request(path, Request::Pause(true)), Ok(Response::Ok)) {
                return daemon;
            }
            if daemon.thread.as_ref().unwrap().is_finished() {
                let result = daemon.thread.take().unwrap().join();
                panic!("daemon exited before viewer attachment: {result:?}");
            }
            assert!(Instant::now() < deadline, "daemon did not start");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn stop(mut self) {
        assert!(matches!(
            request(&self.path, Request::Stop).unwrap(),
            Response::Ok
        ));
        self.thread.take().unwrap().join().unwrap().unwrap();
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = request(&self.path, Request::Stop);
            let _ = thread.join();
        }
    }
}

fn render(
    world: &Path,
    output: &Path,
    expected_live: bool,
    expected_tick: u64,
    panel: &str,
    headless: bool,
) {
    let log_path = output.with_extension("log");
    let log = std::fs::File::create(&log_path).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_bestiary-viewer"));
    command
        .arg(world)
        .arg("--smoke")
        .arg(output)
        .arg("--panel")
        .arg(panel.to_lowercase());
    if headless {
        command.arg("--headless");
    }
    let mut viewer = Viewer(
        command
            .stdout(Stdio::from(log.try_clone().unwrap()))
            .stderr(Stdio::from(log))
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if let Some(status) = viewer.0.try_wait().unwrap() {
            assert!(
                status.success(),
                "viewer failed: {status}\n{}",
                std::fs::read_to_string(&log_path).unwrap()
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "viewer render timed out\n{}",
            std::fs::read_to_string(&log_path).unwrap()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let bytes = std::fs::read(output).expect("viewer did not write a PNG");
    let image = Image::from_buffer(
        &bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::default(),
        RenderAssetUsages::default(),
    )
    .expect("viewer wrote an invalid PNG")
    .try_into_dynamic()
    .unwrap()
    .to_rgb8();
    assert!(image.width() >= 720 && image.height() >= 480);
    let colors: BTreeSet<_> = image.pixels().step_by(97).map(|pixel| pixel.0).collect();
    assert!(colors.len() > 32, "viewer rendered a blank frame");
    if headless {
        let heading_pixels = (0..90)
            .flat_map(|y| (0..300).map(move |x| (x, y)))
            .filter(|(x, y)| {
                let [r, g, b] = image.get_pixel(*x, *y).0;
                r >= 50 && g as u16 > r as u16 * 13 / 10 && b > r
            })
            .count();
        assert!(
            heading_pixels > 50,
            "captured scene omitted the teal BESTIARY interface heading"
        );
    }

    println!(
        "rendered {}x{} frame with {} sampled colors",
        image.width(),
        image.height(),
        colors.len()
    );
    let report: serde_json::Value = serde_json::from_slice(
        &std::fs::read(output.with_extension("png.json"))
            .expect("viewer did not write smoke metadata"),
    )
    .unwrap();
    assert_eq!(report["live"], expected_live);
    assert_eq!(report["panel"], panel);
    if panel == "Inspector" {
        assert_eq!(report["anatomy"], true);
        assert!(
            report["muscle_meshes"].as_u64().unwrap() > 0,
            "anatomy must render physical muscles"
        );
    }
    assert_eq!(report["tick"], expected_tick);
    assert!(report["frames"].as_u64().unwrap() >= 1);
}

fn live_snapshot(path: &Path) -> World {
    let Response::Snapshot(world) = request(path, Request::Snapshot).unwrap() else {
        panic!("expected daemon snapshot")
    };
    *world
}

/// A hidden native window still needs a graphics adapter and display server.
/// Run this explicit GPU acceptance test with scripts/check-viewer-render.
#[test]
#[ignore = "requires a graphics adapter and native display; run scripts/check-viewer-render"]
fn headless_render_smoke_and_viewer_attach_detach_preserve_state_hash() {
    let directory = TemporaryDirectory(
        std::env::temp_dir().join(format!("bestiary-viewer-render-{}", std::process::id())),
    );
    std::fs::create_dir(&directory.0).unwrap();
    let path = directory.0.join("world.db");
    let mut config = Config::bundled();
    config.world.grid = 8;
    let mut world = World::new(42, config, 4).unwrap();
    {
        let mut store = Store::create(&path).unwrap();
        store.save(&world).unwrap();
        for _ in 0..200 {
            world.step();
            if world.tick % 100 == 0 {
                store.save(&world).unwrap();
            }
        }
    }

    let before_bytes = std::fs::read(&path).unwrap();
    let before_hash = world.state_hash();
    render(
        &path,
        &directory.0.join("offline.png"),
        false,
        world.tick,
        "World",
        false,
    );
    for panel in [
        "World",
        "Inspector",
        "Archive",
        "Phylogeny",
        "Charts",
        "Highlights",
    ] {
        render(
            &path,
            &directory.0.join(format!("{panel}.png")),
            false,
            world.tick,
            panel,
            true,
        );
    }
    let after: World = Store::open(&path, true).unwrap().load().unwrap();
    assert_eq!(before_bytes, std::fs::read(&path).unwrap());
    assert_eq!(before_hash, after.state_hash());

    let daemon = Daemon::start(&path);
    let before = live_snapshot(&path);
    render(
        &path,
        &directory.0.join("attached.png"),
        true,
        before.tick,
        "World",
        false,
    );
    assert_eq!(before.state_hash(), live_snapshot(&path).state_hash());
    daemon.stop();
    let saved: World = Store::open(&path, true).unwrap().load().unwrap();
    assert_eq!(before.state_hash(), saved.state_hash());
}
