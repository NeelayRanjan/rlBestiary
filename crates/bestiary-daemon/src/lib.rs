//! Local process boundary. Wall-clock pacing and IPC stay outside simulation crates.
use bestiary_archive::store::Store;
use bestiary_core::genome::Genome;
use bestiary_world::{hex, World};
use fs2::FileExt;
use interprocess::local_socket::{
    prelude::*, GenericFilePath, GenericNamespaced, ListenerNonblockingMode, ListenerOptions,
    Stream,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
const MAX_FRAME: usize = 128 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Request {
    Snapshot,
    Status,
    Events {
        since: u64,
    },
    Replay(String),
    Pause(bool),
    Speed(f32),
    Stop,
    Release {
        genome: Genome,
        position: [f32; 2],
        count: usize,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Status {
    pub tick: u64,
    pub population: usize,
    pub species: usize,
    pub state_hash: String,
    pub paused: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Response {
    Snapshot(Box<World>),
    Status(Status),
    Events(Vec<(u64, bestiary_world::EcologyEvent)>),
    Replay(Vec<u8>),
    Ok,
    Error(String),
}
pub fn status(world: &World, paused: bool) -> Status {
    Status {
        tick: world.tick,
        population: world.agents.len(),
        species: world
            .registry
            .species
            .values()
            .filter(|s| s.population > 0)
            .count(),
        state_hash: hex(&world.state_hash()),
        paused,
    }
}
pub fn write_frame<T: Serialize>(mut stream: impl Write, message: &T) -> Result<()> {
    let bytes = postcard::to_allocvec(message)?;
    if bytes.len() > MAX_FRAME {
        return Err("IPC frame too large".into());
    }
    stream.write_all(&(bytes.len() as u32).to_le_bytes())?;
    stream.write_all(&bytes)?;
    stream.flush()?;
    Ok(())
}
pub fn read_frame<T: DeserializeOwned>(mut stream: impl Read) -> Result<T> {
    let mut header = [0; 4];
    stream.read_exact(&mut header)?;
    let len = u32::from_le_bytes(header) as usize;
    if len > MAX_FRAME {
        return Err("IPC frame exceeds limit".into());
    }
    let mut bytes = vec![0; len];
    stream.read_exact(&mut bytes)?;
    let (value, tail) = postcard::take_from_bytes(&bytes)?;
    if !tail.is_empty() {
        return Err("trailing IPC data".into());
    }
    Ok(value)
}
pub fn endpoint(path: &Path) -> Result<String> {
    let canonical = std::fs::canonicalize(path)?;
    let hash = bestiary_core::digest(&canonical.to_string_lossy().as_ref())?;
    let name = format!("bestiary-{}", &hex(&hash)[..20]);
    if GenericNamespaced::is_supported() {
        Ok(name)
    } else {
        Ok(std::env::temp_dir()
            .join(format!("{name}.sock"))
            .to_string_lossy()
            .into_owned())
    }
}
pub fn request(path: &Path, request: Request) -> Result<Response> {
    let endpoint = endpoint(path)?;
    let name = if GenericNamespaced::is_supported() {
        endpoint.clone().to_ns_name::<GenericNamespaced>()?
    } else {
        endpoint.clone().to_fs_name::<GenericFilePath>()?
    };
    let mut stream = Stream::connect(name)?;
    write_frame(&mut stream, &request)?;
    read_frame(&mut stream)
}
pub fn load_snapshot(path: &Path) -> Result<World> {
    if let Ok(Response::Snapshot(w)) = request(path, Request::Snapshot) {
        return Ok(*w);
    }
    Store::open(path, true)?.load()
}
pub fn run(path: &Path, until: Option<u64>, snapshot_every: u64, speed: Option<f32>) -> Result<()> {
    if snapshot_every == 0 || speed.is_some_and(|s| !s.is_finite() || s <= 0.0) {
        return Err("invalid snapshot interval or speed".into());
    }
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path.with_extension("db.lock"))?;
    lock.try_lock_exclusive()
        .map_err(|_| "this world already has a writer")?;
    let mut store = Store::open(path, false)?;
    let mut world: World = store.load()?;
    for genome in world.genomes.values_mut() {
        genome.replays.clear();
    }
    let endpoint = endpoint(path)?;
    if !GenericNamespaced::is_supported() && Path::new(&endpoint).exists() {
        std::fs::remove_file(&endpoint)?;
    }
    let name = if GenericNamespaced::is_supported() {
        endpoint.clone().to_ns_name::<GenericNamespaced>()?
    } else {
        endpoint.clone().to_fs_name::<GenericFilePath>()?
    };
    let listener = ListenerOptions::new().name(name).create_sync()?;
    listener.set_nonblocking(ListenerNonblockingMode::Accept)?;
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    ctrlc::set_handler(move || signal.store(true, Ordering::SeqCst))?;
    let (sender, receiver) = mpsc::sync_channel::<(Request, mpsc::SyncSender<Response>)>(16);
    let done = stop.clone();
    let server = std::thread::spawn(move || {
        while !done.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok(mut stream) => {
                    if let Err(error) = stream.set_nonblocking(false) {
                        eprintln!("IPC stream setup failed: {error}");
                        continue;
                    }
                    let sender = sender.clone();
                    std::thread::spawn(move || {
                        if let Ok(request) = read_frame::<Request>(&mut stream) {
                            let (tx, rx) = mpsc::sync_channel(1);
                            if sender.send((request, tx)).is_ok() {
                                if let Ok(response) = rx.recv() {
                                    if let Err(error) = write_frame(&mut stream, &response) {
                                        eprintln!("IPC response failed: {error}");
                                    }
                                }
                            }
                        }
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(_) => break,
            }
        }
    });
    eprintln!("bestiary daemon: {endpoint}");
    let mut paused = false;
    let mut speed = speed;
    let result = (|| -> Result<()> {
        while !stop.load(Ordering::Relaxed) && until.is_none_or(|limit| world.tick < limit) {
            while let Ok((command, reply)) = receiver.try_recv() {
                let response = match command {
                    Request::Snapshot => Response::Snapshot(Box::new(world.clone())),
                    Request::Status => Response::Status(status(&world, paused)),
                    Request::Events { since } => {
                        match store.event_history::<bestiary_world::EcologyEvent>() {
                            Ok(events) => Response::Events(
                                events
                                    .into_iter()
                                    .filter(|(tick, _)| *tick >= since)
                                    .collect(),
                            ),
                            Err(e) => Response::Error(e.to_string()),
                        }
                    }
                    Request::Replay(id) => match store.replay(&id) {
                        Ok(bytes) => Response::Replay(bytes),
                        Err(e) => Response::Error(e.to_string()),
                    },
                    Request::Pause(value) => {
                        paused = value;
                        Response::Ok
                    }
                    Request::Speed(value) if value.is_finite() && value >= 0.0 => {
                        speed = if value == 0.0 { None } else { Some(value) };
                        Response::Ok
                    }
                    Request::Speed(_) => Response::Error("invalid speed".into()),
                    Request::Stop => {
                        stop.store(true, Ordering::Relaxed);
                        Response::Ok
                    }
                    Request::Release {
                        genome,
                        position,
                        count,
                    } => match world.release(genome, position, count) {
                        Ok(()) => {
                            store.save(&world)?;
                            Response::Ok
                        }
                        Err(e) => Response::Error(e),
                    },
                };
                let _ = reply.send(response);
            }
            if stop.load(Ordering::Relaxed) {
                break;
            }
            if paused {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            world.step();
            store.events(world.tick, &world.ecological_events)?;
            if world.tick % snapshot_every == 0 {
                store.save(&world)?;
                for genome in world.genomes.values_mut() {
                    genome.replays.clear();
                }
            }
            if let Some(speed) = speed {
                std::thread::sleep(Duration::from_secs_f32(0.1 / speed));
            }
        }
        store.save(&world)?;
        Ok(())
    })();
    stop.store(true, Ordering::Relaxed);
    let _ = server.join();
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ipc_codec_roundtrip_and_invalid_lengths() {
        let mut bytes = vec![];
        write_frame(&mut bytes, &Request::Pause(true)).unwrap();
        assert!(matches!(
            read_frame::<Request>(bytes.as_slice()).unwrap(),
            Request::Pause(true)
        ));
        assert!(read_frame::<Request>(&[255u8; 4][..]).is_err());
        assert!(read_frame::<Request>(&bytes[..3]).is_err());
    }
}
