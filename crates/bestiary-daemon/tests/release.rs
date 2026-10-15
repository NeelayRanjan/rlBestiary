use bestiary_archive::store::Store;
use bestiary_daemon::{request, Request, Response};
use bestiary_sim::config::Config;
use bestiary_world::{EcologyEvent, World};
use std::time::{Duration, Instant};

struct Child(std::process::Child);
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn paused_release_persists_discovery_events_without_advancing_tick_or_creating_resources() {
    let directory = std::env::temp_dir().join(format!("bestiary-release-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("world.db");
    let mut config = Config::bundled();
    config.world.grid = 8;
    let world = World::new(42, config, 0).unwrap();
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
            panic!("daemon exited before pause: {status}");
        }
        assert!(Instant::now() < deadline, "daemon did not start");
        std::thread::sleep(Duration::from_millis(10));
    }
    let snapshot = || match request(&path, Request::Snapshot).unwrap() {
        Response::Snapshot(world) => *world,
        other => panic!("expected snapshot, got {other:?}"),
    };
    let before = snapshot();
    let mut genome = before.genomes.values().next().unwrap().genome.clone();
    let mut hashes = vec![];
    for _ in 0..2 {
        // Each slightly heavier genome must emit a new RecordSize discovery.
        // Two releases at the same tick also exercise the persisted event prefix.
        for bone in &mut genome.bones {
            bone.density *= 1.05;
        }
        let hash = genome.hash();
        hashes.push(hash);
        let response = request(
            &path,
            Request::Release {
                genome: genome.clone(),
                position: [1.0, 1.0],
                count: 1,
            },
        )
        .unwrap();
        assert!(matches!(response, Response::Ok), "{response:?}");
        let after = snapshot();
        assert_eq!(after.tick, before.tick);
        assert_eq!(after.totals(), before.totals());
        assert!(after.conserved());
        assert_eq!(after.agents.len(), hashes.len());
        assert!(after.ecological_events.iter().any(
            |event| matches!(event, EcologyEvent::RecordSize { genome, .. } if *genome == hash)
        ));
        let store = Store::open(&path, true).unwrap();
        let stored: World = store.load().unwrap();
        assert_eq!(stored.state_hash(), after.state_hash());
        let events: Vec<(u64, EcologyEvent)> = store.event_history().unwrap();
        let current: Vec<_> = events
            .iter()
            .filter(|(tick, _)| *tick == before.tick)
            .map(|(_, event)| event)
            .collect();
        assert_eq!(
            postcard::to_allocvec(&current).unwrap(),
            postcard::to_allocvec(&after.ecological_events).unwrap()
        );
        for hash in &hashes {
            assert_eq!(
                current
                    .iter()
                    .filter(|event| matches!(event,
                    EcologyEvent::RecordSize { genome, .. } if genome == hash))
                    .count(),
                1,
                "each discovery must be persisted once"
            );
        }
        let Response::Events(ipc_events) =
            request(&path, Request::Events { since: before.tick }).unwrap()
        else {
            panic!("expected event history")
        };
        assert_eq!(
            postcard::to_allocvec(&ipc_events).unwrap(),
            postcard::to_allocvec(
                &events
                    .into_iter()
                    .filter(|(tick, _)| *tick >= before.tick)
                    .collect::<Vec<_>>()
            )
            .unwrap()
        );
    }
    let after = snapshot();
    let Response::Status(status) = request(&path, Request::Status).unwrap() else {
        panic!("expected status")
    };
    assert!(status.paused);
    assert_eq!(status.tick, before.tick);
    assert!(matches!(
        request(&path, Request::Stop).unwrap(),
        Response::Ok
    ));
    assert!(child.0.wait().unwrap().success());
    // Resuming advances and clears the world's transient event buffer, but the
    // released discoveries must remain in the queryable history.
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_bestiary-daemon"))
        .arg(&path)
        .arg((after.tick + 1).to_string())
        .status()
        .unwrap();
    assert!(status.success());
    let store = Store::open(&path, true).unwrap();
    let resumed: World = store.load().unwrap();
    assert_eq!(resumed.tick, after.tick + 1);
    assert!(resumed.conserved());
    let events: Vec<(u64, EcologyEvent)> = store.event_history().unwrap();
    for hash in hashes {
        assert_eq!(
            events
                .iter()
                .filter(|(tick, event)| *tick == before.tick
                    && matches!(event,
            EcologyEvent::RecordSize { genome, .. } if *genome == hash))
                .count(),
            1
        );
    }
    drop(store);
    std::fs::remove_dir_all(directory).unwrap();
}
