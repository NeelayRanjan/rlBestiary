mod anatomy;
mod benchmark;
mod data;
mod designer;
mod designer_handles;
mod export;
mod playback;
mod scene;
mod ui;
mod ui_designer;
use bestiary_sim::{
    combine::{Replay, TrialKind},
    genome::Creature,
    physics::Pose,
};
use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
    window::PresentMode,
};
use data::{Feed, Job, Message, ReplayKey, Snapshot};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tab {
    World,
    Inspector,
    Designer,
    Archive,
    Phylogeny,
    Charts,
    Highlights,
}
impl Tab {
    fn is_body(self) -> bool {
        matches!(self, Self::Inspector | Self::Designer)
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Overlay {
    Biomass,
    Temperature,
    Moisture,
    Pheromone(usize),
    Species,
    Predation,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CameraMode {
    Free,
    Follow,
    Director,
}
#[derive(Resource)]
struct View {
    snapshot: Arc<Snapshot>,
    designer: designer::Designer,
    tab: Tab,
    overlay: Overlay,
    selected: Option<u64>,
    genome: [u8; 32],
    mode: CameraMode,
    target: Vec3,
    radius: f32,
    yaw: f32,
    pitch: f32,
    dirty: bool,
    paused: bool,
    speed: f32,
    timer: Timer,
    pending_refresh: bool,
    replays: BTreeMap<ReplayKey, Replay>,
    pending: BTreeSet<ReplayKey>,
    unavailable: BTreeSet<ReplayKey>,
    live_preview: bool,
    anatomy: bool,
    trial: TrialKind,
    error: String,
    axes: [usize; 2],
    slices: [u8; 7],
    replay_frame: usize,
    playing: bool,
    play_time: f32,
    world_play_time: f32,
    highlight_tick: u64,
    clip: Option<Clip>,
    clip_start: f32,
    clip_seconds: f32,
    smoke: Option<PathBuf>,
    benchmark: bool,
    frame: u64,
    benchmark_start: Option<std::time::Instant>,
    benchmark_frames: u64,
    benchmark_visible: [usize; 2],
    canvas_rect: [f32; 4],
    canvas_scale: f32,
}
impl View {
    fn body_genome(&self) -> Option<&bestiary_core::genome::Genome> {
        if self.tab == Tab::Designer {
            Some(&self.designer.genome)
        } else {
            self.snapshot
                .world
                .genomes
                .get(&self.genome)
                .map(|g| &g.genome)
        }
    }
    fn body_hash(&self) -> [u8; 32] {
        if self.tab == Tab::Designer {
            self.designer.hash
        } else {
            self.genome
        }
    }
}
struct Clip {
    dir: PathBuf,
    schedule: playback::ClipSchedule,
    resume_playing: bool,
    exit_after: bool,
    first_frame_at: u64,
    metadata: serde_json::Value,
}
#[derive(Resource, Clone)]
struct CaptureCompletion(Arc<AtomicBool>);
#[derive(Default)]
struct Inspector {
    hash: Option<[u8; 32]>,
    creature: Option<Creature>,
    poses: Vec<Pose>,
    accumulator: f32,
}
fn main() {
    if let Err(error) = run() {
        eprintln!("bestiary-viewer: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let path = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or("usage: bestiary-viewer WORLD.db [--smoke OUT.png] [--benchmark]")?;
    let snapshot = Arc::new(data::read(std::path::Path::new(path))?);
    let (selected, genome) = data::initial_selection(&snapshot.world)?;
    let extent = if args.iter().any(|s| s == "--benchmark") {
        60.0
    } else {
        snapshot.world.config.world.grid as f32 * snapshot.world.config.world.cell_metres
    };
    let smoke = args
        .iter()
        .position(|s| s == "--smoke")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    let benchmark = args.iter().any(|s| s == "--benchmark");
    let headless = benchmark || args.iter().any(|s| s == "--headless");
    let tab = match args
        .iter()
        .position(|s| s == "--panel")
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
    {
        None | Some("world") => Tab::World,
        Some("inspector") => Tab::Inspector,
        Some("designer") => Tab::Designer,
        Some("archive") => Tab::Archive,
        Some("phylogeny") => Tab::Phylogeny,
        Some("charts") => Tab::Charts,
        Some("highlights") => Tab::Highlights,
        Some(_) => return Err("unknown panel".into()),
    };
    let appearance = if let Some(i) = args.iter().position(|s| s == "--appearance") {
        anatomy::Appearance::parse(
            &std::fs::read_to_string(args.get(i + 1).ok_or("--appearance needs a RON path")?)
                .map_err(|e| e.to_string())?,
        )?
    } else {
        anatomy::Appearance::parse(include_str!("../../../config/viewer.ron"))?
    };
    let feed = Feed::new(PathBuf::from(path));
    let paused = snapshot.paused;
    let current_physics = snapshot.physics_revision == Some(bestiary_sim::SIMULATION_REVISION);
    let highlight_tick = snapshot.world.tick;
    let slices = snapshot
        .world
        .archive
        .cells
        .keys()
        .next()
        .copied()
        .unwrap_or([0; 7]);
    let designer = designer::Designer::new(
        snapshot.world.genomes[&genome].genome.clone(),
        &snapshot.world.config,
    );
    let mut view = View {
        designer,
        selected,
        snapshot,
        genome,
        tab,
        overlay: Overlay::Biomass,
        mode: CameraMode::Free,
        target: Vec3::new(extent * 0.5, 0.0, extent * 0.5),
        radius: if tab.is_body() {
            5.0
        } else if benchmark {
            extent * 1.7
        } else {
            extent * 0.85
        },
        yaw: 0.6,
        pitch: 0.72,
        dirty: true,
        paused,
        speed: 1.0,
        timer: Timer::from_seconds(1.0, TimerMode::Repeating),
        pending_refresh: false,
        replays: BTreeMap::new(),
        pending: BTreeSet::new(),
        unavailable: BTreeSet::new(),
        live_preview: current_physics,
        anatomy: !args.iter().any(|s| s == "--colliders"),
        trial: TrialKind::Sprint,
        error: String::new(),
        axes: [0, 2],
        slices,
        replay_frame: 0,
        playing: true,
        play_time: 0.0,
        world_play_time: 0.0,
        highlight_tick,
        clip: None,
        clip_start: 0.0,
        clip_seconds: 4.0,
        smoke: smoke.clone(),
        benchmark,
        frame: 0,
        benchmark_start: None,
        benchmark_frames: 0,
        benchmark_visible: [usize::MAX; 2],
        canvas_rect: [0.0, 0.0, 1440.0, 960.0],
        canvas_scale: 1.0,
    };
    let exporting = export::configure(&args, &mut view)?;
    let completion = CaptureCompletion(Arc::new(AtomicBool::new(false)));
    let mut app = App::new();
    if exporting {
        app.insert_resource(export::CameraOnly);
    }
    if headless {
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: bevy::window::ExitCondition::DontExit,
                    close_when_requested: false,
                })
                .disable::<bevy::winit::WinitPlugin>(),
        )
        .add_plugins(bevy::app::ScheduleRunnerPlugin::run_loop(
            std::time::Duration::ZERO,
        ));
    } else {
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bestiary · a living archive".into(),
                resolution: (1440.0, 960.0).into(),
                visible: smoke.is_none() && !exporting,
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }));
    }
    app.insert_resource(bevy::winit::WinitSettings {
        focused_mode: bevy::winit::UpdateMode::Continuous,
        unfocused_mode: bevy::winit::UpdateMode::Continuous,
    })
    .add_plugins(bevy_egui::EguiPlugin)
    .insert_resource(ClearColor(Color::srgb(0.025, 0.04, 0.055)))
    .insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 600.0,
    })
    .insert_resource(appearance)
    .insert_resource(view)
    .insert_resource(completion.clone())
    .insert_resource(feed)
    .insert_non_send_resource(Inspector::default())
    .add_systems(Startup, scene::setup)
    .add_systems(
        PostUpdate,
        scene::viewport.after(bevy::render::camera::CameraUpdateSystem),
    )
    .add_systems(
        Update,
        (
            poll,
            ui::panel.run_if(not(resource_exists::<export::CameraOnly>)),
            designer_handles::draw,
            prepare_clip_frame,
            simulate_inspector,
            scene::camera,
            scene::rebuild,
            scene::animate,
            scene::animate_tissue,
            scene::joints_and_muscles,
            scene::contact_ground,
            capture,
        )
            .chain(),
    );
    if headless {
        benchmark::install(&mut app);
    }
    let exit_status = app.run();
    if !matches!(exit_status, AppExit::Success) {
        return Err("viewer stopped with an error".into());
    }
    if (benchmark || smoke.is_some() || exporting) && !completion.0.load(Ordering::Relaxed) {
        return Err("viewer stopped before completing the requested render or benchmark".into());
    }
    Ok(())
}
fn select_genome(view: &mut View, hash: [u8; 32], feed: &Feed) {
    view.selected = view.selected.filter(|id| {
        view.snapshot
            .world
            .history
            .get(id)
            .is_some_and(|life| life.genome == hash)
    });
    view.genome = hash;
    view.play_time = 0.0;
    view.replay_frame = 0;
    request_replay(view, (hash, view.trial), feed);
    view.dirty = true;
}
fn request_replay(view: &mut View, key: ReplayKey, feed: &Feed) {
    if view.replays.contains_key(&key) || view.unavailable.contains(&key) {
        return;
    }
    if let Some(replay) = view.snapshot.world.genomes.get(&key.0).and_then(|g| {
        g.replays
            .iter()
            .filter_map(|bytes| Replay::decode(bytes).ok())
            .find(|r| r.kind == key.1)
    }) {
        view.replays.insert(key, replay);
    } else if view.pending.insert(key) {
        let _ = feed.tx.send(Job::Replay(key));
    }
}
fn focus(view: &mut View, id: u64, feed: &Feed) {
    if let Some(a) = view.snapshot.world.agents.iter().find(|a| a.id == id) {
        let hash = a.genome;
        let position = a.position;
        view.selected = Some(id);
        view.target = Vec3::new(position[0], 0.5, position[1]);
        view.radius = if view.tab == Tab::Inspector {
            5.0
        } else {
            12.0
        };
        view.mode = CameraMode::Follow;
        select_genome(view, hash, feed);
    } else if let Some(life) = view.snapshot.world.history.get(&id) {
        let hash = life.genome;
        view.selected = Some(id);
        select_genome(view, hash, feed);
        view.tab = Tab::Inspector;
        view.radius = 5.0;
    }
}
fn poll(mut view: ResMut<View>, feed: Res<Feed>, time: Res<Time>) {
    // Preserve both the world snapshot and its available replay set throughout
    // an export. Pending feed results are applied after the last PNG is saved.
    if view.clip.is_some() {
        return;
    }
    let messages: Vec<_> = feed.rx.lock().expect("feed lock").try_iter().collect();
    for message in messages {
        match message {
            Message::Snapshot(snapshot) => {
                let mut directed_genome = None;
                if snapshot.revision != view.snapshot.revision {
                    view.unavailable.clear();
                    if view.mode == CameraMode::Director {
                        if let Some((tick, event)) = snapshot
                            .events
                            .iter()
                            .rev()
                            .find(|(t, e)| *t > view.highlight_tick && data::notable(e))
                        {
                            view.highlight_tick = *tick;
                            if let Some((id, hash)) = data::event_subject(&snapshot.world, event) {
                                view.selected = id;
                                directed_genome = Some(hash);
                                if let Some(a) =
                                    snapshot.world.agents.iter().find(|a| Some(a.id) == id)
                                {
                                    view.target = Vec3::new(a.position[0], 0.0, a.position[1]);
                                    view.tab = Tab::World;
                                    view.radius = 24.0;
                                } else {
                                    view.tab = Tab::Inspector;
                                    view.radius = 5.0;
                                }
                            }
                        }
                    }
                    view.dirty = true;
                }
                view.paused = snapshot.paused;
                view.snapshot = snapshot;
                if let Some(hash) = directed_genome {
                    select_genome(&mut view, hash, &feed);
                }
                view.pending_refresh = false;
            }
            Message::Replay(hash, replay) => {
                view.replays.insert(hash, replay);
                view.pending.remove(&hash);
            }
            Message::ReplayMissing(key) => {
                view.pending.remove(&key);
                view.unavailable.insert(key);
            }
            Message::Error(error) => {
                view.error = error;
                view.pending_refresh = false;
            }
            Message::CommandDone => {}
            Message::Combined(generation, result) => {
                if view.designer.accept_evaluation(generation, result) {
                    if let Some(evaluation) = view.designer.evaluation.clone() {
                        for trial in evaluation.trials {
                            if let Some(replay) = trial.replay {
                                view.replays
                                    .insert((evaluation.genome_hash, replay.kind), replay);
                            }
                        }
                        view.designer.preview = true;
                        view.live_preview = false;
                        view.playing = true;
                        view.trial = TrialKind::Sprint;
                        view.play_time = 0.0;
                        view.replay_frame = 0;
                        view.dirty = true;
                    }
                }
            }
            Message::Released(result) => {
                view.designer.status = match result {
                    Ok(()) => "Founding population released into the running world.".into(),
                    Err(error) => format!("Release failed: {error}"),
                };
                let _ = feed.tx.send(Job::Refresh);
            }
        }
    }
    view.timer.tick(time.delta());
    if view.timer.just_finished()
        && !view.pending_refresh
        && !view.benchmark
        && view.smoke.is_none()
        && view.clip.is_none()
    {
        view.pending_refresh = feed.tx.send(Job::Refresh).is_ok();
    }
    view.world_play_time = playback::advance_world_clock(
        view.world_play_time,
        time.delta_secs(),
        view.clip.is_none() && (view.benchmark || (view.snapshot.live && !view.paused)),
    );
    if view.playing && view.clip.is_none() {
        view.play_time += time.delta_secs();
    }
    if let Some(replay) = view.replays.get(&(view.body_hash(), view.trial)) {
        if view.playing && view.clip.is_none() {
            view.replay_frame =
                playback::frame_index(view.play_time, replay.dt, replay.frames.len());
        }
    }
}
fn prepare_clip_frame(mut view: ResMut<View>) {
    let Some((frame, clamp)) = view.clip.as_mut().and_then(|clip| {
        clip.schedule
            .prepare()
            .map(|frame| (frame, clip.exit_after))
    }) else {
        return;
    };
    // This runs after the UI can start an export and before the scene chooses
    // poses, including the very first captured frame.
    view.play_time = frame.seconds;
    if view.tab == Tab::World {
        view.world_play_time = frame.seconds;
    }
    if let Some(replay) = view.replays.get(&(view.body_hash(), view.trial)) {
        view.replay_frame = if clamp {
            playback::clamped_frame_index(frame.seconds, replay.dt, replay.frames.len())
        } else {
            playback::frame_index(frame.seconds, replay.dt, replay.frames.len())
        };
    }
}
fn simulate_inspector(view: Res<View>, time: Res<Time>, mut inspector: NonSendMut<Inspector>) {
    if !view.tab.is_body() {
        return;
    }
    if inspector.hash != Some(view.body_hash()) {
        inspector.creature = view
            .body_genome()
            .and_then(|genome| Creature::new(genome.clone(), &view.snapshot.world.config).ok());
        inspector.hash = Some(view.body_hash());
        inspector.accumulator = 0.0;
    }
    if view.playing
        && view.live_preview
        && view.clip.is_none()
        && (view.tab != Tab::Designer || view.designer.preview)
    {
        inspector.accumulator += time.delta_secs().min(0.05);
        while inspector.accumulator >= 1.0 / 240.0 {
            if let Some(creature) = &mut inspector.creature {
                creature.step();
            }
            inspector.accumulator -= 1.0 / 240.0;
        }
    }
    if let Some(creature) = &inspector.creature {
        inspector.poses = creature.arena.poses();
    }
}
#[allow(clippy::too_many_arguments)]
fn capture(
    mut commands: Commands,
    mut view: ResMut<View>,
    mut exit: EventWriter<AppExit>,
    proxies: Query<&ViewVisibility, With<scene::Proxy>>,
    bodies: Query<(&scene::BodyPart, &ViewVisibility)>,
    muscles: Query<&scene::MuscleVisual>,
    completion: Res<CaptureCompletion>,
    target: Option<Res<benchmark::Target>>,
) {
    let screenshot = || {
        target
            .as_ref()
            .map_or_else(Screenshot::primary_window, |target| {
                Screenshot::image(target.0.clone())
            })
    };
    view.frame += 1;
    if view.benchmark && view.frame == 120 {
        view.benchmark_start = Some(std::time::Instant::now());
    }
    if view.benchmark && view.frame > 120 {
        view.benchmark_visible[0] =
            view.benchmark_visible[0].min(proxies.iter().filter(|v| v.get()).count());
        view.benchmark_visible[1] = view.benchmark_visible[1].min(
            bodies
                .iter()
                .filter(|(p, v)| p.index == 0 && v.get())
                .count(),
        );
        view.benchmark_frames += 1;
        if view.benchmark_frames == 600 {
            let fps = 600.0 / view.benchmark_start.unwrap().elapsed().as_secs_f64();
            let passes =
                fps >= 60.0 && view.benchmark_visible[0] >= 2000 && view.benchmark_visible[1] >= 20;
            println!(
                "{}",
                serde_json::json!({"proxies":view.benchmark_visible[0],"full_bodies":view.benchmark_visible[1],"fps":fps,"passes":passes})
            );
            completion.0.store(true, Ordering::Relaxed);
            exit.send(if passes {
                AppExit::Success
            } else {
                AppExit::error()
            });
        }
    }
    if view.frame == 90 {
        if let Some(path) = view.smoke.take() {
            let completion = completion.clone();
            let report = serde_json::json!({"live":view.snapshot.live,"panel":format!("{:?}",view.tab),"tick":view.snapshot.world.tick,"frames":view.frame,"anatomy":view.anatomy,"muscle_meshes":muscles.iter().count(),"body_hash":view.body_hash(),"body_parts":bodies.iter().map(|(part, _)| part.index).collect::<BTreeSet<_>>().len(),"visible_body_parts":bodies.iter().filter(|(_, visible)|visible.get()).map(|(part, _)|part.index).collect::<BTreeSet<_>>().len()});
            commands.spawn(screenshot()).observe(
                move |event: Trigger<ScreenshotCaptured>, mut exit: EventWriter<AppExit>| {
                    let result = event
                        .event()
                        .0
                        .clone()
                        .try_into_dynamic()
                        .map_err(|e| e.to_string())
                        .and_then(|image| image.to_rgb8().save(&path).map_err(|e| e.to_string()))
                        .and_then(|_| {
                            std::fs::write(path.with_extension("png.json"), report.to_string())
                                .map_err(|e| e.to_string())
                        });
                    if let Err(e) = result {
                        eprintln!("capture failed: {e}");
                        exit.send(AppExit::error());
                    } else {
                        completion.0.store(true, Ordering::Relaxed);
                        exit.send(AppExit::Success);
                    }
                },
            );
        }
    }
    let frame_number = view.frame;
    if let Some(clip) = view.clip.as_mut() {
        if let Some(frame) = (frame_number >= clip.first_frame_at)
            .then(|| clip.schedule.begin_capture())
            .flatten()
        {
            let path = clip.dir.join(format!("frame-{:05}.png", frame.index));
            let exit_after = clip.exit_after;
            commands.spawn(screenshot()).observe(
                move |event: Trigger<ScreenshotCaptured>,
                      mut view: ResMut<View>,
                      mut exit: EventWriter<AppExit>| {
                    let result = event
                        .event()
                        .0
                        .clone()
                        .try_into_dynamic()
                        .map_err(|e| e.to_string())
                        .and_then(|image| image.to_rgb8().save(&path).map_err(|e| e.to_string()));
                    if let Err(e) = result {
                        view.error = e;
                        view.clip = None;
                        if exit_after {
                            exit.send(AppExit::error());
                        }
                    } else if let Some(clip) = view.clip.as_mut() {
                        clip.schedule.complete(frame.index);
                    }
                },
            );
        }
        if clip.schedule.is_complete() {
            let playing = clip.resume_playing;
            let exit_after = clip.exit_after;
            clip.metadata["complete"] = true.into();
            let saved = std::fs::write(clip.dir.join("clip.json"), clip.metadata.to_string());
            view.clip = None;
            view.playing = playing;
            match saved {
                Ok(()) if exit_after => {
                    completion.0.store(true, Ordering::Relaxed);
                    exit.send(AppExit::Success);
                }
                Err(error) => {
                    view.error = error.to_string();
                    if exit_after {
                        exit.send(AppExit::error());
                    }
                }
                _ => {}
            }
        }
    }
}
