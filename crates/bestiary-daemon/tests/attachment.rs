use bestiary_archive::store::Store;
use bestiary_daemon::{request, Request, Response};
use bestiary_sim::config::Config;
use bestiary_world::World;
use std::time::{Duration, Instant};
struct Child(std::process::Child);
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[test]
fn viewer_attach_detach_does_not_perturb_state_hash() {
    let directory = std::env::temp_dir().join(format!("bestiary-ipc-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("world.db");
    let world = World::new(42, Config::bundled(), 2).unwrap();
    Store::create(&path).unwrap().save(&world).unwrap();
    let mut child = Child(
        std::process::Command::new(env!("CARGO_BIN_EXE_bestiary-daemon"))
            .arg(&path)
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if matches!(request(&path, Request::Pause(true)), Ok(Response::Ok)) {
            break;
        }
        if let Some(status) = child.0.try_wait().unwrap() {
            panic!("daemon exited before attachment: {status}");
        }
        assert!(Instant::now() < deadline, "daemon did not start");
        std::thread::sleep(Duration::from_millis(10));
    }
    let Response::Snapshot(first) = request(&path, Request::Snapshot).unwrap() else {
        panic!("expected snapshot")
    };
    let Response::Snapshot(second) = request(&path, Request::Snapshot).unwrap() else {
        panic!("expected snapshot")
    };
    assert_eq!(first.state_hash(), second.state_hash());
    assert!(matches!(
        request(&path, Request::Stop).unwrap(),
        Response::Ok
    ));
    assert!(child.0.wait().unwrap().success());
    let stored: World = Store::open(&path, true).unwrap().load().unwrap();
    assert_eq!(first.state_hash(), stored.state_hash());
    // A clean stop must allow a second daemon to bind the exact same endpoint.
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_bestiary-daemon"))
        .arg(&path)
        .arg((stored.tick + 1).to_string())
        .status()
        .unwrap();
    assert!(status.success());
    let restarted: World = Store::open(&path, true).unwrap().load().unwrap();
    assert_eq!(restarted.tick, stored.tick + 1);
    let mut expected = stored;
    expected.step();
    assert_eq!(restarted.state_hash(), expected.state_hash());
    std::fs::remove_dir_all(directory).unwrap();
}
