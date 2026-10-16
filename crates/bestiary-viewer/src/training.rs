//! Training panel state and its background worker. All physics runs in
//! `bestiary_sim::training`; this module only schedules it and keeps results.
use bestiary_core::genome::Genome;
use bestiary_sim::{
    combine::Replay,
    config::Config,
    training::{GenerationReport, Task, Training, TrainingSettings},
};
use bevy::prelude::Resource;
use std::collections::BTreeMap;
use std::sync::{
    mpsc::{self, TryRecvError},
    Mutex,
};

/// Training settings for the viewer: `--training PATH` overrides the embedded
/// `config/training.ron`, mirroring `--appearance`.
pub fn settings_from_args(args: &[String]) -> Result<TrainingSettings, String> {
    match args.iter().position(|a| a == "--training") {
        None => Ok(TrainingSettings::bundled()),
        Some(i) => {
            let path = args.get(i + 1).ok_or("--training needs a RON path")?;
            let source = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
            TrainingSettings::parse(&source).map_err(|e| format!("training: {e}"))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Preset(usize),
    Designer,
    Selected,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub generation: u32,
    pub best: f32,
    pub median: f32,
}

pub struct TrainingView {
    pub source: Source,
    pub task: Task,
    pub settings: TrainingSettings,
    pub seed: u64,
    /// Identifies the current run; reports from older runs are ignored.
    pub run: u64,
    pub running: bool,
    pub history: Vec<Summary>,
    /// Champion hash per generation, oldest first.
    champion_hashes: Vec<[u8; 32]>,
    /// Each distinct champion's genome and replay, stored once.
    champion_store: BTreeMap<[u8; 32], (Genome, Replay)>,
    /// The generation being played back in the arena.
    pub latest: Option<GenerationReport>,
    /// The newest report, waiting for the current playback loop to finish.
    pending: Option<GenerationReport>,
    /// Generation whose champion is shown; `None` follows the latest.
    pub shown: Option<u32>,
    pub show_all: bool,
    /// Playback position in seconds within the shown replays.
    pub time: f32,
    pub status: String,
}
impl TrainingView {
    pub fn new() -> Self {
        Self {
            source: Source::Preset(0),
            task: Task::Sprint,
            settings: TrainingSettings::bundled(),
            seed: 42,
            run: 0,
            running: false,
            history: vec![],
            champion_hashes: vec![],
            champion_store: BTreeMap::new(),
            latest: None,
            pending: None,
            shown: None,
            show_all: true,
            time: 0.0,
            status: String::new(),
        }
    }
    /// Discards results and moves to a new run id so late reports are ignored.
    pub fn reset(&mut self) {
        self.run += 1;
        self.running = false;
        self.history.clear();
        self.champion_hashes.clear();
        self.champion_store.clear();
        self.latest = None;
        self.pending = None;
        self.shown = None;
        self.time = 0.0;
    }
    pub fn accept(&mut self, run: u64, report: GenerationReport) -> bool {
        if run != self.run {
            return false;
        }
        self.history.push(Summary {
            generation: report.generation,
            best: report.best,
            median: report.median,
        });
        let hash = report.champion.hash();
        self.champion_hashes.push(hash);
        self.champion_store
            .entry(hash)
            .or_insert_with(|| (report.champion.clone(), report.champion_replay.clone()));
        self.pending = Some(report);
        true
    }
    pub fn generations(&self) -> usize {
        self.champion_hashes.len()
    }
    #[cfg(test)]
    pub fn stored_champions(&self) -> usize {
        self.champion_store.len()
    }
    pub fn champion(&self, generation: usize) -> Option<(&Genome, &Replay)> {
        let hash = self.champion_hashes.get(generation)?;
        self.champion_store.get(hash).map(|(g, r)| (g, r))
    }
    /// Seconds one playback loop lasts, including a one-second hold at the end.
    fn loop_seconds(&self) -> f32 {
        self.slots()
            .iter()
            .map(|(_, r, _)| r.frames.len() as f32 * r.dt)
            .fold(0.0, f32::max)
            + 1.0
    }
    /// Advances playback. A waiting generation is swapped in only when nothing
    /// is shown or the current loop has finished, so every run plays out in
    /// full. Returns true when the arena must be rebuilt.
    pub fn advance(&mut self, seconds: f32) -> bool {
        self.time += seconds;
        let finished = self.time > self.loop_seconds();
        if finished {
            self.time = 0.0;
        }
        if (finished || self.latest.is_none()) && self.pending.is_some() {
            self.latest = self.pending.take();
            self.time = 0.0;
            return true;
        }
        false
    }
    /// Bodies to draw, best first; the flag marks the champion.
    pub fn slots(&self) -> Vec<(&Genome, &Replay, bool)> {
        match (self.shown, &self.latest) {
            (None, Some(latest)) if self.show_all => latest
                .genomes
                .iter()
                .zip(&latest.replays)
                .enumerate()
                .map(|(i, (g, r))| (g, r, i == 0))
                .collect(),
            (Some(n), _) => self
                .champion(n as usize)
                .map(|(g, r)| vec![(g, r, true)])
                .unwrap_or_default(),
            (None, Some(latest)) => vec![(&latest.champion, &latest.champion_replay, true)],
            (None, None) => vec![],
        }
    }
}
impl Default for TrainingView {
    fn default() -> Self {
        Self::new()
    }
}

pub enum Command {
    Start {
        run: u64,
        source: Box<Genome>,
        task: Task,
        settings: Box<TrainingSettings>,
        config: Box<Config>,
        seed: u64,
    },
    Pause,
    Resume,
    Stop,
}
pub enum Event {
    Report(u64, Box<GenerationReport>),
    Failed(u64, String),
}

/// Owns the running `Training` on its own thread; commands take effect between
/// generations, so a generation in flight always completes.
#[derive(Resource)]
pub struct Worker {
    pub tx: mpsc::Sender<Command>,
    pub rx: Mutex<mpsc::Receiver<Event>>,
}
impl Worker {
    pub fn new() -> Self {
        let (tx, commands) = mpsc::channel::<Command>();
        let (events, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut current: Option<(u64, Training)> = None;
            let mut paused = false;
            loop {
                let next = if current.is_none() || paused {
                    match commands.recv() {
                        Ok(command) => Some(command),
                        Err(_) => return,
                    }
                } else {
                    match commands.try_recv() {
                        Ok(command) => Some(command),
                        Err(TryRecvError::Empty) => None,
                        Err(TryRecvError::Disconnected) => return,
                    }
                };
                match next {
                    Some(Command::Start {
                        run,
                        source,
                        task,
                        settings,
                        config,
                        seed,
                    }) => {
                        paused = false;
                        current = match Training::new(*source, task, *settings, *config, seed) {
                            Ok(t) => Some((run, t)),
                            Err(e) => {
                                let _ = events.send(Event::Failed(run, e));
                                None
                            }
                        };
                        continue;
                    }
                    Some(Command::Pause) => {
                        paused = true;
                        continue;
                    }
                    Some(Command::Resume) => {
                        paused = false;
                        continue;
                    }
                    Some(Command::Stop) => {
                        current = None;
                        continue;
                    }
                    None => {}
                }
                if let Some((run, training)) = &mut current {
                    let event = match training.next_generation(true) {
                        Ok(r) => Event::Report(*run, Box::new(r)),
                        Err(e) => {
                            let failed = Event::Failed(*run, e);
                            current = None;
                            failed
                        }
                    };
                    if events.send(event).is_err() {
                        return;
                    }
                }
            }
        });
        Self {
            tx,
            rx: Mutex::new(rx),
        }
    }
}
impl Default for Worker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn report(generation: u32, best: f32) -> GenerationReport {
        let c = Config::bundled();
        let g = Genome::starter(&c.genome);
        let replay = Replay {
            kind: bestiary_sim::combine::TrialKind::Sprint,
            dt: 1.0 / 240.0,
            frames: vec![],
        };
        GenerationReport {
            generation,
            best,
            median: best / 2.0,
            worst: 0.0,
            champion: g.clone(),
            champion_replay: replay.clone(),
            runs: vec![],
            genomes: vec![g.clone(), g],
            replays: vec![replay.clone(), replay],
        }
    }
    #[test]
    fn training_view_accepts_reports_only_from_the_current_run() {
        let mut v = TrainingView::new();
        v.run = 2;
        assert!(!v.accept(1, report(0, 1.0)), "stale run accepted");
        assert!(v.accept(2, report(0, 1.0)));
        assert!(v.accept(2, report(1, 1.5)));
        assert_eq!(
            v.history.iter().map(|s| s.best).collect::<Vec<_>>(),
            vec![1.0, 1.5]
        );
        assert_eq!(v.generations(), 2);
    }
    #[test]
    fn training_settings_come_from_the_command_line_file() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            settings_from_args(&args(&["world.db"])).unwrap(),
            TrainingSettings::bundled()
        );
        let dir = std::env::temp_dir().join(format!("bestiary-training-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let good = dir.join("good.ron");
        std::fs::write(
            &good,
            include_str!("../../../config/training.ron").replace("population: 50", "population: 7"),
        )
        .unwrap();
        let bad = dir.join("bad.ron");
        std::fs::write(&bad, "(population: 0)").unwrap();
        let parsed = settings_from_args(&args(&["world.db", "--training", good.to_str().unwrap()]));
        assert_eq!(parsed.unwrap().population, 7);
        assert!(settings_from_args(&args(&["w", "--training", bad.to_str().unwrap()])).is_err());
        assert!(settings_from_args(&args(&["w", "--training", "/nonexistent.ron"])).is_err());
        assert!(settings_from_args(&args(&["w", "--training"])).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn champions_are_stored_once_per_genome() {
        let mut v = TrainingView::new();
        for generation in 0..3 {
            v.accept(v.run, report(generation, 1.0));
        }
        assert_eq!(v.generations(), 3);
        assert_eq!(v.stored_champions(), 1);
        assert!(v.champion(2).is_some() && v.champion(3).is_none());
    }
    #[test]
    fn new_generations_wait_for_the_playback_loop() {
        let mut v = TrainingView::new();
        let mut first = report(0, 1.0);
        first.replays[0].frames = vec![vec![]; 240];
        first.replays[1].frames = vec![vec![]; 240];
        assert!(v.accept(v.run, first));
        assert!(v.advance(0.0), "first generation is shown at once");
        assert!(v.accept(v.run, report(1, 2.0)));
        assert_eq!(v.history.len(), 2, "the chart updates immediately");
        assert_eq!(v.latest.as_ref().unwrap().generation, 0);
        assert!(!v.advance(0.5), "mid-loop: keep playing generation 0");
        assert_eq!(v.latest.as_ref().unwrap().generation, 0);
        assert!(v.advance(2.0), "loop ended: swap in generation 1");
        assert_eq!(v.latest.as_ref().unwrap().generation, 1);
        assert_eq!(v.time, 0.0);
    }
    #[test]
    fn training_view_slots_show_generation_or_champion() {
        let mut v = TrainingView::new();
        v.accept(v.run, report(0, 1.0));
        v.advance(0.0);
        v.show_all = true;
        assert_eq!(v.slots().len(), 2);
        assert!(v.slots()[0].2 && !v.slots()[1].2);
        v.shown = Some(0);
        assert_eq!(v.slots().len(), 1);
        v.reset();
        assert!(v.slots().is_empty() && v.history.is_empty());
    }
    fn start(w: &Worker, run: u64) {
        let c = Config::bundled();
        let settings = TrainingSettings {
            population: 2,
            ..TrainingSettings::bundled()
        };
        w.tx.send(Command::Start {
            run,
            source: Box::new(Genome::starter(&c.genome)),
            task: Task::Sprint,
            settings: Box::new(settings),
            config: Box::new(c),
            seed: 1,
        })
        .unwrap();
    }
    fn next(rx: &mpsc::Receiver<Event>) -> (u64, u32) {
        match rx
            .recv_timeout(std::time::Duration::from_secs(120))
            .unwrap()
        {
            Event::Report(run, r) => (run, r.generation),
            Event::Failed(_, e) => panic!("{e}"),
        }
    }
    fn drain(rx: &mpsc::Receiver<Event>) {
        while rx.recv_timeout(std::time::Duration::from_secs(5)).is_ok() {}
    }
    #[test]
    fn worker_resumes_after_pause_from_the_next_generation() {
        let w = Worker::new();
        start(&w, 4);
        let rx = w.rx.lock().unwrap();
        assert_eq!(next(&rx), (4, 0));
        w.tx.send(Command::Pause).unwrap();
        drain(&rx);
        w.tx.send(Command::Resume).unwrap();
        let (run, generation) = next(&rx);
        assert_eq!(run, 4);
        assert!(generation >= 1);
    }
    #[test]
    fn worker_stop_then_start_reports_only_the_new_run() {
        let w = Worker::new();
        start(&w, 1);
        let rx = w.rx.lock().unwrap();
        assert_eq!(next(&rx).0, 1);
        w.tx.send(Command::Stop).unwrap();
        drain(&rx);
        start(&w, 2);
        assert_eq!(next(&rx), (2, 0));
        let mut view = TrainingView::new();
        view.run = 2;
        assert!(!view.accept(1, report(0, 1.0)), "stale run must be ignored");
    }
    #[test]
    fn worker_reports_generations_and_stops_on_pause() {
        let c = Config::bundled();
        let w = Worker::new();
        let settings = TrainingSettings {
            population: 2,
            ..TrainingSettings::bundled()
        };
        let source = Genome::starter(&c.genome);
        w.tx.send(Command::Start {
            run: 9,
            source: Box::new(source),
            task: Task::Sprint,
            settings: Box::new(settings),
            config: Box::new(c),
            seed: 1,
        })
        .unwrap();
        let rx = w.rx.lock().unwrap();
        let first = rx
            .recv_timeout(std::time::Duration::from_secs(120))
            .unwrap();
        assert!(matches!(first, Event::Report(9, ref r) if r.generation == 0));
        w.tx.send(Command::Pause).unwrap();
        // At most the in-flight generation completes after Pause.
        let mut after = 0;
        while rx.recv_timeout(std::time::Duration::from_secs(20)).is_ok() {
            after += 1;
        }
        assert!(after <= 1, "{after} reports after pause");
    }
}
