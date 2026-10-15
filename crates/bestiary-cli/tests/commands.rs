use bestiary_archive::store::Store;
use bestiary_daemon::{request, Request, Response};
use bestiary_sim::config::Config;
use bestiary_world::World;
use std::{
    process::{Child, Command},
    time::{Duration, Instant},
};

struct Daemon(Child);
impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn cli(args: &[&str]) -> serde_json::Value {
    let out = Command::new(env!("CARGO_BIN_EXE_bestiary"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
#[test]
fn fresh_world_status_genome_combine_replay_and_phylogeny_commands() {
    let dir = std::env::temp_dir().join(format!("bestiary-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("world.db");
    let replay = dir.join("sprint.rpl");
    let path = path.to_str().unwrap();
    let replay = replay.to_str().unwrap();
    let initial = cli(&[
        "world",
        "new",
        "--seed",
        "9",
        "--population",
        "4",
        "--out",
        path,
        "--json",
    ]);
    assert_eq!(initial["population"], 4);
    {
        let store = Store::open(std::path::Path::new(path), true).unwrap();
        let world: World = store.load().unwrap();
        let events = store
            .event_history::<bestiary_world::EcologyEvent>()
            .unwrap();
        assert!(
            !events.is_empty(),
            "initial body discoveries must be persisted at tick zero"
        );
        assert!(events.iter().all(|(tick, _)| *tick == 0));
        assert_eq!(
            postcard::to_allocvec(&events.iter().map(|(_, event)| event).collect::<Vec<_>>())
                .unwrap(),
            postcard::to_allocvec(&world.ecological_events).unwrap()
        );
    }
    assert_eq!(
        cli(&["world", "status", path, "--json"])["state_hash"],
        initial["state_hash"]
    );
    assert!(cli(&["world", "attach", path, "--json"])["endpoint"].is_string());
    assert_eq!(
        cli(&["genome", "validate", "serpent", "--json"])["valid"],
        true
    );
    let result = cli(&["combine", "grazer", "--replay", replay, "--json"]);
    assert_eq!(result["exploit_suspect"], false);
    assert!(
        cli(&["replay", replay, "--json"])["frames"]
            .as_u64()
            .unwrap()
            > 1
    );
    assert!(cli(&["phylo", path, "--json"]).is_object());
    assert!(!Command::new(env!("CARGO_BIN_EXE_bestiary"))
        .arg("invalid")
        .output()
        .unwrap()
        .status
        .success());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn world_status_reports_live_pause_state() {
    let dir = std::env::temp_dir().join(format!("bestiary-live-status-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("world.db");
    Store::create(&path)
        .unwrap()
        .save(&World::new(42, Config::bundled(), 2).unwrap())
        .unwrap();
    let mut daemon = Daemon(
        Command::new(env!("CARGO_BIN_EXE_bestiary"))
            .args(["world", "run", path.to_str().unwrap(), "--speed", "1"])
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if matches!(request(&path, Request::Pause(true)), Ok(Response::Ok)) {
            break;
        }
        if let Some(status) = daemon.0.try_wait().unwrap() {
            panic!("daemon exited before status query: {status}");
        }
        assert!(Instant::now() < deadline, "daemon did not start");
        std::thread::sleep(Duration::from_millis(10));
    }
    let Response::Status(live) = request(&path, Request::Status).unwrap() else {
        panic!("expected live daemon status")
    };
    let status = cli(&["world", "status", path.to_str().unwrap(), "--json"]);
    assert_eq!(status["paused"], true);
    assert_eq!(status["tick"], live.tick);
    assert_eq!(status["state_hash"], live.state_hash);
    let text = Command::new(env!("CARGO_BIN_EXE_bestiary"))
        .args(["world", "status", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(text.status.success());
    assert!(String::from_utf8(text.stdout)
        .unwrap()
        .contains("paused=true"));
    assert!(matches!(
        request(&path, Request::Stop).unwrap(),
        Response::Ok
    ));
    assert!(daemon.0.wait().unwrap().success());
    let offline = cli(&["world", "status", path.to_str().unwrap(), "--json"]);
    assert_eq!(offline["paused"], false);
    assert_eq!(offline["state_hash"], live.state_hash);
    std::fs::remove_dir_all(dir).unwrap();
}
