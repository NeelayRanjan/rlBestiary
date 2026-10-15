use bestiary_archive::store::{Store, TimeSample};
use bestiary_daemon::{request, Request, Response};
use bestiary_sim::combine::{Replay, TrialKind};
use bestiary_world::{EcologyEvent, World};
use bevy::prelude::Resource;
use std::{
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
};

pub struct Snapshot {
    pub revision: [u8; 32],
    pub world: World,
    pub samples: Vec<TimeSample>,
    pub events: Vec<(u64, EcologyEvent)>,
    pub live: bool,
    pub paused: bool,
    pub physics_revision: Option<u32>,
}
pub type ReplayKey = ([u8; 32], TrialKind);
/// Keep the initial body and the selected living creature tied to the same genome.
/// The genome map also contains historical and never-born mutation candidates.
pub fn initial_selection(world: &World) -> Result<(Option<u64>, [u8; 32]), String> {
    if let Some(agent) = world.agents.first() {
        if world.genomes.contains_key(&agent.genome) {
            return Ok((Some(agent.id), agent.genome));
        }
        return Err("selected creature's genome is missing".into());
    }
    let hash = world
        .archive
        .cells
        .values()
        .map(|elite| elite.genome)
        .find(|hash| world.genomes.contains_key(hash))
        .or_else(|| world.genomes.keys().next().copied())
        .ok_or("world contains no genomes")?;
    Ok((None, hash))
}

pub fn matching_creature(world: &World, selected: Option<u64>, hash: [u8; 32]) -> Option<u64> {
    world
        .agents
        .iter()
        .find(|agent| Some(agent.id) == selected && agent.genome == hash)
        .map(|agent| agent.id)
}

pub fn read(path: &Path) -> Result<Snapshot, String> {
    let store = Store::open(path, true).map_err(|e| e.to_string())?;
    let (world, live) = match request(path, Request::Snapshot) {
        Ok(Response::Snapshot(w)) => (*w, true),
        _ => (store.load().map_err(|e| e.to_string())?, false),
    };
    let mut snapshot = Snapshot::new(
        world,
        live,
        matches!(request(path, Request::Status), Ok(Response::Status(s)) if s.paused),
        store.samples().map_err(|e| e.to_string())?,
        store.event_history().map_err(|e| e.to_string())?,
    );
    snapshot.physics_revision = store.simulation_revision().map_err(|e| e.to_string())?;
    Ok(snapshot)
}
impl Snapshot {
    fn new(
        world: World,
        live: bool,
        paused: bool,
        mut samples: Vec<TimeSample>,
        mut events: Vec<(u64, EcologyEvent)>,
    ) -> Self {
        // The daemon may advance between the IPC snapshot and SQLite reads.
        // Keep every displayed event and sample inside this snapshot's horizon.
        samples.retain(|sample| sample.tick <= world.tick);
        events.retain(|(tick, _)| *tick <= world.tick);
        Self {
            revision: world.state_hash(),
            physics_revision: Some(bestiary_sim::SIMULATION_REVISION),
            world,
            samples,
            events,
            live,
            paused,
        }
    }
}

pub enum Job {
    Refresh,
    Command(Request),
    Replay(ReplayKey),
}
pub enum Message {
    Snapshot(Arc<Snapshot>),
    Replay(ReplayKey, Replay),
    ReplayMissing(ReplayKey),
    Error(String),
    CommandDone,
}
#[derive(Resource)]
pub struct Feed {
    pub tx: mpsc::Sender<Job>,
    pub rx: Mutex<mpsc::Receiver<Message>>,
}
impl Feed {
    pub fn new(path: PathBuf) -> Self {
        let (tx, jobs) = mpsc::channel();
        let (messages, rx) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(job) = jobs.recv() {
                let result = match job {
                    Job::Refresh => read(&path).map(|s| Message::Snapshot(Arc::new(s))),
                    Job::Command(command) => match request(&path, command) {
                        Ok(Response::Error(e)) => Err(e),
                        Ok(_) => Ok(Message::CommandDone),
                        Err(e) => Err(e.to_string()),
                    },
                    Job::Replay(key) => Ok(load_replay(&path, key)
                        .map(|r| Message::Replay(key, r))
                        .unwrap_or(Message::ReplayMissing(key))),
                };
                if messages
                    .send(result.unwrap_or_else(Message::Error))
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            tx,
            rx: Mutex::new(rx),
        }
    }
}
fn load_replay(path: &Path, (hash, kind): ReplayKey) -> Option<Replay> {
    let store = Store::open(path, true).ok()?;
    // Failed stand gates may omit sprint. Identify trials from their recorded kind,
    // never from a positional index in the compact replay list.
    (0..2).find_map(|index| {
        let bytes = store
            .replay(&format!("{}-{index}", bestiary_core::hex(&hash)))
            .ok()?;
        let replay = Replay::decode(&bytes).ok()?;
        (replay.kind == kind).then_some(replay)
    })
}
pub fn number(value: f64) -> String {
    if !value.is_finite() {
        return "—".into();
    }
    if value.abs() >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0)
    } else if value.abs() >= 1000.0 {
        format!("{:.1}k", value / 1000.0)
    } else {
        format!("{value:.1}")
    }
}
pub fn bins(points: &[(u64, f32)], width: usize) -> Vec<(u64, f32)> {
    if width == 0 || points.is_empty() {
        return vec![];
    }
    let stride = points.len().div_ceil(width);
    points
        .chunks(stride)
        .map(|chunk| {
            (
                chunk.last().unwrap().0,
                chunk.iter().map(|p| p.1).sum::<f32>() / chunk.len() as f32,
            )
        })
        .collect()
}
pub fn median(mut values: Vec<f32>) -> f32 {
    values.retain(|v| v.is_finite());
    values.sort_by(f32::total_cmp);
    if values.is_empty() {
        0.0
    } else {
        values[values.len() / 2]
    }
}
pub fn species_population_at(samples: &[TimeSample], species: u64, tick: u64) -> usize {
    samples
        .iter()
        .rev()
        .find(|s| s.tick <= tick)
        .and_then(|s| s.species_populations.iter().find(|(id, _)| *id == species))
        .map(|(_, n)| *n)
        .unwrap_or(0)
}
/// Resolve the actual event subject; historical genomes remain inspectable even
/// if natural selection has removed every living carrier.
pub fn event_subject(world: &World, event: &EcologyEvent) -> Option<(Option<u64>, [u8; 32])> {
    let by_id = |id: u64| {
        world.history.get(&id).map(|life| {
            (
                world.agents.iter().any(|a| a.id == id).then_some(id),
                life.genome,
            )
        })
    };
    let by_genome = |hash: [u8; 32]| {
        world.genomes.contains_key(&hash).then(|| {
            (
                world.agents.iter().find(|a| a.genome == hash).map(|a| a.id),
                hash,
            )
        })
    };
    match event {
        EcologyEvent::FirstSwimmer { genome }
        | EcologyEvent::FirstFlyerAdjacentGait { genome, .. }
        | EcologyEvent::ArchiveCell { genome, .. }
        | EcologyEvent::RecordSize { genome, .. }
        | EcologyEvent::Exploit { genome } => by_genome(*genome),
        EcologyEvent::SpeciesBorn { species } | EcologyEvent::SpeciesExtinct { species } => world
            .agents
            .iter()
            .find(|a| a.species == *species)
            .map(|a| (Some(a.id), a.genome))
            .or_else(|| {
                world
                    .registry
                    .species
                    .get(species)
                    .and_then(|s| by_genome(s.founder))
            }),
        EcologyEvent::RecordLifespan { creature, .. }
        | EcologyEvent::Birth { creature, .. }
        | EcologyEvent::Death { creature } => by_id(*creature),
        EcologyEvent::Predation { attacker, .. }
        | EcologyEvent::FirstPredation { attacker, .. }
        | EcologyEvent::FirstPheromoneGuidedHunt { attacker, .. } => by_id(*attacker),
        _ => None,
    }
}

pub fn event_title(event: &EcologyEvent) -> String {
    match event {
        EcologyEvent::SpeciesBorn { species } => format!("Species {species} established"),
        EcologyEvent::SpeciesExtinct { species } => format!("Species {species} extinct"),
        EcologyEvent::FirstPredation { .. } => "First predation".into(),
        EcologyEvent::FirstSwimmer { .. } => "First swimming body".into(),
        EcologyEvent::FirstFlyerAdjacentGait { .. } => "First repeated airborne gait".into(),
        EcologyEvent::FirstPheromoneGuidedHunt { .. } => "First pheromone-guided hunt".into(),
        EcologyEvent::ArchiveCell { cell, .. } => format!("New archive cell {cell:?}"),
        EcologyEvent::Fragmentation { before, after } => {
            format!("Land fragmented: {before} → {after}")
        }
        EcologyEvent::RecordSize { mass, .. } => format!("Record body mass: {mass:.2} kg"),
        EcologyEvent::RecordLifespan { age, .. } => format!("Record lifespan: {age} ticks"),
        EcologyEvent::PopulationCrash { before, after } => {
            format!("Population crash: {before} → {after}")
        }
        EcologyEvent::MassExtinction => "Mass extinction".into(),
        EcologyEvent::Exploit { .. } => "Energy audit flagged a genome".into(),
        EcologyEvent::Shock { kind } => format!("Environmental shock {kind}"),
        other => format!("{other:?}"),
    }
}
pub fn notable(event: &EcologyEvent) -> bool {
    !matches!(
        event,
        EcologyEvent::Birth { .. }
            | EcologyEvent::Death { .. }
            | EcologyEvent::Geography
            | EcologyEvent::Predation { .. }
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_snapshot_excludes_samples_and_events_from_a_newer_database_save() {
        let mut world = World::new(42, bestiary_sim::config::Config::bundled(), 1).unwrap();
        world.tick = 20;
        let samples = [10, 20, 30]
            .into_iter()
            .map(|tick| TimeSample {
                tick,
                ..Default::default()
            })
            .collect();
        let events = vec![
            (
                10,
                EcologyEvent::FirstPredation {
                    attacker: 0,
                    victim: 1,
                },
            ),
            (20, EcologyEvent::MassExtinction),
            (30, EcologyEvent::SpeciesBorn { species: 7 }),
        ];
        let snapshot = Snapshot::new(world, true, false, samples, events);
        assert_eq!(
            snapshot.samples.iter().map(|s| s.tick).collect::<Vec<_>>(),
            [10, 20]
        );
        assert_eq!(
            snapshot.events.iter().map(|e| e.0).collect::<Vec<_>>(),
            [10, 20]
        );
        assert_eq!(snapshot.revision, snapshot.world.state_hash());
    }
    #[test]
    fn director_follows_event_genome_and_retains_extinct_archive_subjects() {
        let mut world = World::new(42, bestiary_sim::config::Config::bundled(), 2).unwrap();
        let mut genome = world.genomes[&world.agents[0].genome].genome.clone();
        genome.bones[0].color = [0.1, 0.2, 0.3];
        let hash = genome.hash();
        let old_revision = world.state_hash();
        let tick = world.tick;
        world.release(genome, [1.0, 1.0], 1).unwrap();
        assert_eq!(world.tick, tick);
        assert_ne!(
            world.state_hash(),
            old_revision,
            "paused releases need a new viewer revision"
        );
        let id = world.agents.last().unwrap().id;
        let event = EcologyEvent::FirstSwimmer { genome: hash };
        assert_eq!(event_subject(&world, &event), Some((Some(id), hash)));
        world.agents.retain(|a| a.id != id);
        assert_eq!(event_subject(&world, &event), Some((None, hash)));
        assert_eq!(event_subject(&world, &EcologyEvent::MassExtinction), None);
    }
    #[test]
    fn historical_first_predation_and_guided_hunt_keep_the_recorded_attacker() {
        let mut world = World::new(42, bestiary_sim::config::Config::bundled(), 2).unwrap();
        let attacker = world.agents[0].id;
        let hash = world.agents[0].genome;
        let victim = world.agents[1].id;
        let events = [
            EcologyEvent::FirstPredation { attacker, victim },
            EcologyEvent::FirstPheromoneGuidedHunt {
                attacker,
                victim,
                decision_tick: 2,
                pheromone_channels: 1,
            },
        ];
        world.ecological_events.clear();
        world.tick = 100;
        for event in &events {
            assert_eq!(event_subject(&world, event), Some((Some(attacker), hash)));
            assert!(notable(event));
        }
        world.agents.retain(|agent| agent.id != attacker);
        for event in &events {
            assert_eq!(event_subject(&world, event), Some((None, hash)));
        }
        let gait = EcologyEvent::FirstFlyerAdjacentGait {
            genome: hash,
            cycles: 2,
            max_clearance: 0.2,
        };
        assert_eq!(event_subject(&world, &gait), Some((Some(victim), hash)));
        assert!(notable(&gait));
        assert_eq!(event_title(&gait), "First repeated airborne gait");
    }
    #[test]
    fn phylogeny_population_uses_historical_samples_not_current_species_assignments() {
        let samples = vec![
            TimeSample {
                tick: 10,
                species_populations: vec![(1, 3)],
                ..Default::default()
            },
            TimeSample {
                tick: 20,
                species_populations: vec![(1, 1), (2, 2)],
                ..Default::default()
            },
        ];
        assert_eq!(species_population_at(&samples, 1, 9), 0);
        assert_eq!(species_population_at(&samples, 1, 15), 3);
        assert_eq!(species_population_at(&samples, 1, 20), 1);
        assert_eq!(species_population_at(&samples, 2, 15), 0);
        assert_eq!(species_population_at(&samples, 2, 25), 2);
    }
    #[test]
    fn inspector_initial_body_matches_living_creature_not_first_historical_genome() {
        let mut world = World::new(42, bestiary_sim::config::Config::bundled(), 1).unwrap();
        let living = world.agents[0].clone();
        let mut candidate = world.genomes[&living.genome].clone();
        // Find an actual unrelated genome that sorts before the living creature.
        for i in 0..256 {
            candidate.genome.bones[0].color[0] = i as f32 / 255.0;
            let hash = candidate.genome.hash();
            if hash < living.genome {
                world.genomes.insert(hash, candidate.clone());
                break;
            }
        }
        assert_ne!(world.genomes.keys().next(), Some(&living.genome));
        assert_eq!(
            initial_selection(&world).unwrap(),
            (Some(living.id), living.genome)
        );
        assert_eq!(
            matching_creature(&world, Some(living.id), living.genome),
            Some(living.id)
        );
        assert_eq!(
            matching_creature(&world, Some(living.id), candidate.genome.hash()),
            None
        );

        world.agents.clear();
        let (selected, hash) = initial_selection(&world).unwrap();
        assert_eq!(selected, None);
        assert!(world
            .archive
            .cells
            .values()
            .any(|elite| elite.genome == hash));
        world.archive.cells.clear();
        assert!(initial_selection(&world).is_ok());
        world.genomes.clear();
        assert!(initial_selection(&world).is_err());
    }

    #[test]
    fn formatting_and_chart_binning_handle_empty_and_nonfinite_data() {
        assert_eq!(number(1500.0), "1.5k");
        assert_eq!(number(f64::NAN), "—");
        assert!(bins(&[], 5).is_empty());
        assert!(bins(&[(0, 1.0)], 0).is_empty());
        assert_eq!(
            bins(&[(1, 1.0), (2, 3.0), (3, 5.0)], 2),
            vec![(2, 2.0), (3, 5.0)]
        );
        assert_eq!(median(vec![3.0, f32::NAN, 1.0, 2.0]), 2.0);
    }
}
