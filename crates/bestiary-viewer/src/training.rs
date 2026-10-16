//! Training panel state and its background worker. All physics runs in
//! `bestiary_sim::training`; this module only schedules it and keeps results.
use bestiary_core::genome::Genome;
use bestiary_sim::{
    combine::Replay,
    config::Config,
    training::{GenerationReport, Task, Training, TrainingSettings},
};
use bevy::prelude::Resource;
use std::sync::{
    mpsc::{self, TryRecvError},
    Mutex,
};

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
    /// Champion genome and replay per generation, oldest first.
    pub champions: Vec<(Genome, Replay)>,
    pub latest: Option<GenerationReport>,
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
            champions: vec![],
            latest: None,
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
        self.champions.clear();
        self.latest = None;
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
        self.champions
            .push((report.champion.clone(), report.champion_replay.clone()));
        self.latest = Some(report);
        self.time = 0.0;
        true
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
                .champions
                .get(n as usize)
                .map(|(g, r)| vec![(g, r, true)])
                .unwrap_or_default(),
            (None, Some(_)) => self
                .champions
                .last()
                .map(|(g, r)| vec![(g, r, true)])
                .unwrap_or_default(),
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
        assert_eq!(v.champions.len(), 2);
    }
    #[test]
    fn training_view_slots_show_generation_or_champion() {
        let mut v = TrainingView::new();
        v.accept(v.run, report(0, 1.0));
        v.show_all = true;
        assert_eq!(v.slots().len(), 2);
        assert!(v.slots()[0].2 && !v.slots()[1].2);
        v.shown = Some(0);
        assert_eq!(v.slots().len(), 1);
        v.reset();
        assert!(v.slots().is_empty() && v.history.is_empty());
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
