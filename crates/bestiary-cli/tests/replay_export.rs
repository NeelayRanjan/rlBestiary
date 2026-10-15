use bestiary_archive::store::Store;
use bestiary_sim::{
    combine::{Replay, TrialKind},
    config::Config,
};
use bestiary_world::World;
use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

struct Fixture {
    directory: PathBuf,
    world: PathBuf,
    replay: PathBuf,
    id: String,
    genome: String,
}
impl Fixture {
    fn new(name: &str) -> Self {
        let directory =
            std::env::temp_dir().join(format!("bestiary-replay-{name}-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let world = directory.join("world.db");
        let replay = directory.join("recorded replay.rpl");
        let mut config = Config::bundled();
        config.world.grid = 8;
        let simulation = World::new(42, config, 2).unwrap();
        let genome = bestiary_core::hex(simulation.genomes.keys().next().unwrap());
        let id = format!("{genome}-0");
        let mut store = Store::create(&world).unwrap();
        store.save(&simulation).unwrap();
        std::fs::write(&replay, store.replay(&id).unwrap()).unwrap();
        Self {
            directory,
            world,
            replay,
            id,
            genome,
        }
    }
    fn command(&self, binary: &Path, source: &str, world: bool) -> Output {
        let mut command = Command::new(binary);
        command
            .args(["replay", source, "--export"])
            .arg(self.directory.join("PNG export"));
        if world {
            command.arg("--world").arg(&self.world);
        }
        command.arg("--json").output().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn replay_png_export_requires_recorded_anatomy_and_a_fresh_directory() {
    let fixture = Fixture::new("errors");
    let binary = Path::new(env!("CARGO_BIN_EXE_bestiary"));
    let missing = fixture.command(binary, fixture.replay.to_str().unwrap(), false);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("requires --world"));
    let unknown = fixture.directory.join("unknown.rpl");
    std::fs::write(
        &unknown,
        Replay {
            kind: TrialKind::Sprint,
            dt: 0.01,
            frames: vec![vec![]],
        }
        .encode()
        .unwrap(),
    )
    .unwrap();
    let output = fixture.command(binary, unknown.to_str().unwrap(), true);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("anatomy cannot be identified"));
    std::fs::create_dir(fixture.directory.join("PNG export")).unwrap();
    let output = fixture.command(binary, &fixture.id, true);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
}

#[cfg(unix)]
#[test]
fn replay_export_delegates_exact_genome_trial_and_paths_without_a_shell() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new("delegation");
    let binary = fixture.directory.join("bestiary");
    std::fs::copy(env!("CARGO_BIN_EXE_bestiary"), &binary).unwrap();
    let viewer = fixture.directory.join("bestiary-viewer");
    std::fs::write(
        &viewer,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$(dirname \"$0\")/viewer-args.txt\"\nexit 17\n",
    )
    .unwrap();
    std::fs::set_permissions(&viewer, std::fs::Permissions::from_mode(0o755)).unwrap();
    for source in [&fixture.id, fixture.replay.to_str().unwrap()] {
        let result = fixture.command(&binary, source, true);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("renderer failed"));
        let arguments = std::fs::read_to_string(fixture.directory.join("viewer-args.txt")).unwrap();
        let args: Vec<_> = arguments.lines().collect();
        assert_eq!(
            args,
            [
                fixture.world.to_str().unwrap(),
                "--export-replay",
                source,
                "--genome",
                &fixture.genome,
                "--trial",
                "Sprint",
                "--export",
                fixture.directory.join("PNG export").to_str().unwrap(),
                "--headless"
            ]
        );
    }
}

#[cfg(unix)]
#[test]
fn replay_export_rejects_false_completion_without_sequential_png_files() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new("false-completion");
    let binary = fixture.directory.join("bestiary");
    std::fs::copy(env!("CARGO_BIN_EXE_bestiary"), &binary).unwrap();
    let viewer = fixture.directory.join("bestiary-viewer");
    std::fs::write(&viewer, "#!/bin/sh\nmkdir \"$9\"\nprintf '{\"complete\":true,\"frames\":61,\"fps\":30,\"genome\":\"%s\",\"trial\":\"%s\"}\\n' \"$5\" \"$7\" > \"$9/clip.json\"\nexit 0\n").unwrap();
    std::fs::set_permissions(&viewer, std::fs::Permissions::from_mode(0o755)).unwrap();
    let output = fixture.command(&binary, &fixture.id, true);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("PNG frame") && error.contains("missing"),
        "{error}"
    );
}
