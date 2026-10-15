use super::*;
use bestiary_archive::grid::{descriptors, AXES};
use bevy_egui::{egui, EguiContexts};
const TEAL: egui::Color32 = egui::Color32::from_rgb(107, 215, 177);
const GOLD: egui::Color32 = egui::Color32::from_rgb(232, 178, 103);
pub fn panel(
    mut contexts: EguiContexts,
    mut view: ResMut<View>,
    feed: Res<Feed>,
    inspector: NonSend<Inspector>,
) {
    let ctx = contexts.ctx_mut();
    let snapshot = view.snapshot.clone();
    let world = &snapshot.world;
    egui::TopBottomPanel::top("masthead").show(ctx, |ui| {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("BESTIARY")
                    .size(21.0)
                    .strong()
                    .color(TEAL),
            );
            ui.label(egui::RichText::new(" /  a living archive").color(egui::Color32::GRAY));
            ui.separator();
            ui.label(format!("SEED {}", world.seed));
            ui.label(format!("TICK {}", world.tick));
            ui.label(format!("{} creatures", world.agents.len()));
            ui.label(format!(
                "{} species",
                world
                    .registry
                    .species
                    .values()
                    .filter(|s| s.population > 0)
                    .count()
            ));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.colored_label(
                    if snapshot.live { TEAL } else { GOLD },
                    if snapshot.live {
                        "● LIVE DAEMON"
                    } else {
                        "● SAVED WORLD"
                    },
                );
            });
        });
        ui.add_space(6.0);
    });
    egui::SidePanel::left("navigation")
        .default_width(255.0)
        .resizable(view.clip.is_none())
        .show(ctx, |ui| {
            if view.clip.is_some() {
                ui.disable();
            }
            ui.add_space(10.0);
            ui.label(
                egui::RichText::new("OBSERVATORY")
                    .small()
                    .color(egui::Color32::GRAY),
            );
            for (tab, label) in [
                (Tab::World, "World"),
                (Tab::Inspector, "Creature inspector"),
                (Tab::Archive, "Body-plan archive"),
                (Tab::Phylogeny, "Phylogeny"),
                (Tab::Charts, "History"),
                (Tab::Highlights, "Highlights & audits"),
            ] {
                if ui.selectable_label(view.tab == tab, label).clicked() {
                    view.tab = tab;
                    view.dirty = true;
                    if tab == Tab::Inspector {
                        view.radius = 5.0;
                        let hash = view.genome;
                        select_genome(&mut view, hash, &feed);
                    }
                    if tab == Tab::World {
                        view.radius =
                            world.config.world.grid as f32 * world.config.world.cell_metres * 0.85;
                    }
                }
            }
            ui.separator();
            ui.label(
                egui::RichText::new("LIVING CREATURES")
                    .small()
                    .color(egui::Color32::GRAY),
            );
            egui::ScrollArea::vertical()
                .id_salt("creatures")
                .max_height(300.0)
                .show_rows(
                    ui,
                    ui.spacing().interact_size.y,
                    world.agents.len(),
                    |ui, rows| {
                        for index in rows {
                            let a = &world.agents[index];
                            let name = world
                                .registry
                                .species
                                .get(&a.species)
                                .map(|s| s.name.as_str())
                                .unwrap_or("Unregistered");
                            if ui
                                .selectable_label(
                                    view.selected == Some(a.id),
                                    format!("#{}  {name}", a.id),
                                )
                                .clicked()
                            {
                                focus(&mut view, a.id, &feed);
                            }
                        }
                    },
                );
            ui.separator();
            ui.label(
                egui::RichText::new("ARCHIVE COVERAGE")
                    .small()
                    .color(egui::Color32::GRAY),
            );
            let coverage = world.archive.coverage(&world.config.archive);
            ui.add(egui::ProgressBar::new(coverage).text(format!(
                "{} / {} cells  ·  {:.1}%",
                world.archive.cells.len(),
                world.config.archive.total_cells(),
                coverage * 100.0
            )));
            ui.label(format!("{} evaluated genomes", world.genomes.len()));
            ui.label(format!("{} recorded mutations", world.mutations.len()));
            ui.separator();
            ui.colored_label(
                if world.conserved() {
                    TEAL
                } else {
                    egui::Color32::RED
                },
                if world.conserved() {
                    "Resources conserved"
                } else {
                    "Conservation audit failed"
                },
            );
            ui.small("Right-drag to orbit · scroll to zoom\nWASD to move the free camera");
        });
    egui::TopBottomPanel::bottom("transport").show(ctx, |ui| {
        if view.clip.is_some() {
            ui.disable();
        }
        ui.horizontal(|ui| {
            ui.add_enabled_ui(snapshot.live, |ui| {
                let label = if view.paused {
                    "Resume world"
                } else {
                    "Pause world"
                };
                if ui.button(label).clicked() {
                    view.paused = !view.paused;
                    let _ = feed
                        .tx
                        .send(Job::Command(bestiary_daemon::Request::Pause(view.paused)));
                }
                ui.label("Speed");
                if ui
                    .add(
                        egui::DragValue::new(&mut view.speed)
                            .range(0.0..=1000.0)
                            .speed(1.0),
                    )
                    .changed()
                {
                    let _ = feed
                        .tx
                        .send(Job::Command(bestiary_daemon::Request::Speed(view.speed)));
                }
                ui.small("0 = maximum");
            });
            ui.separator();
            for mode in [CameraMode::Free, CameraMode::Follow, CameraMode::Director] {
                ui.selectable_value(&mut view.mode, mode, format!("{mode:?}"));
            }
            ui.separator();
            ui.add(egui::DragValue::new(&mut view.clip_start).range(0.0..=600.0).prefix("From ").suffix(" s"));
            ui.add(egui::DragValue::new(&mut view.clip_seconds).range(0.1..=30.0).prefix("For ").suffix(" s"));
            let readiness = clip_readiness(&view);
            let can_export = view.clip.is_none() && matches!(readiness, ClipReadiness::Ready { .. });
            if ui.add_enabled(can_export, egui::Button::new("Export camera clip")).on_hover_text("Exports recorded animation at 30 fps from the current world snapshot. In the inspector, select a recorded replay first.").clicked() {
                start_clip(&mut view);
            }
            if let Some(clip) = &view.clip {
                ui.label(format!("Exporting {}/{}", clip.schedule.completed_frames(), clip.schedule.frame_count()));
            } else if let Some(message) = readiness.message() {
                ui.small(message);
            }
        });
        if !view.error.is_empty() {
            ui.horizontal(|ui| {
                ui.colored_label(GOLD, &view.error);
                if ui.small_button("Dismiss").clicked() {
                    view.error.clear();
                }
            });
        }
    });
    match view.tab {
        Tab::World => {
            egui::Window::new("Terrain overlay")
                .anchor(egui::Align2::RIGHT_TOP, [-18.0, 70.0])
                .resizable(false)
                .enabled(view.clip.is_none())
                .show(ctx, |ui| {
                    let before = view.overlay;
                    for overlay in [
                        Overlay::Biomass,
                        Overlay::Temperature,
                        Overlay::Moisture,
                        Overlay::Pheromone(0),
                        Overlay::Pheromone(1),
                        Overlay::Pheromone(2),
                        Overlay::Pheromone(3),
                        Overlay::Species,
                        Overlay::Predation,
                    ] {
                        ui.selectable_value(&mut view.overlay, overlay, format!("{overlay:?}"));
                    }
                    if before != view.overlay {
                        view.dirty = true;
                    }
                });
            if let Some(a) = world.agents.iter().find(|a| Some(a.id) == view.selected) {
                egui::Window::new("Selected creature")
                    .anchor(egui::Align2::RIGHT_BOTTOM, [-18.0, -80.0])
                    .enabled(view.clip.is_none())
                    .show(ctx, |ui| {
                        ui.heading(format!("Creature #{}", a.id));
                        ui.label(format!(
                            "Energy {} J",
                            data::number(a.energy as f64 / 1000.0)
                        ));
                        ui.add(egui::ProgressBar::new(a.health).text("Health"));
                        if ui.button("Open body & brain").clicked() {
                            focus(&mut view, a.id, &feed);
                            view.tab = Tab::Inspector;
                            view.radius = 5.0;
                            view.dirty = true;
                        }
                    });
            }
        }
        Tab::Inspector => {
            egui::SidePanel::right("inspector")
                .default_width(410.0)
                .resizable(view.clip.is_none())
                .show(ctx, |ui| {
                    if view.clip.is_some() {
                        ui.disable();
                    }
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.heading("Body & behavior");
                        if snapshot.physics_revision != Some(bestiary_sim::SIMULATION_REVISION) {
                            ui.colored_label(GOLD, "Historical physics: cached traits and replays belong to the earlier evaluator.");
                            ui.small("Live physics re-tests this genome with the current muscle and contact model.");
                        }
                        if view.live_preview {
                            if let Some(creature) = &inspector.creature {
                                let tips: Vec<_> = creature.body.parts.iter().enumerate().filter(|(_, p)| p.parent.is_some() && p.children.iter().all(Option::is_none)).map(|(i, _)| i).collect();
                                let grounded = tips.iter().filter(|i| creature.arena.touches_terrain(creature.arena.parts[**i])).count();
                                ui.label(format!("Grounded limb tips: {grounded} / {}", tips.len()));
                            }
                        }
                        ui.horizontal(|ui| {
                            for (anatomy, label) in [(true, "Anatomy"), (false, "Collision shapes")] {
                                if ui.selectable_value(&mut view.anatomy, anatomy, label).changed() { view.dirty = true; }
                            }
                        });
                        ui.small("Bone · tendon · muscle. Tissue follows the physics attachments.");
                        ui.small(if view.live_preview { "Muscles darken slightly with activation; shortening thickens the belly." } else { "Recorded poses; muscle thickness follows length. Activation was not recorded." });
                        if let Some(id) = data::matching_creature(world, view.selected, view.genome)
                        {
                            ui.label(format!("Living creature #{id}"));
                        } else if let Some(life) = view.selected.and_then(|id| world.history.get(&id)) {
                            ui.label(format!("Creature #{} · born {} · died {:?}", life.id, life.born, life.died));
                        } else {
                            ui.label("Recorded genome · no living individual selected");
                        }
                        ui.monospace(&bestiary_core::hex(&view.genome)[..16]);
                        if let Some(g) = world.genomes.get(&view.genome) {
                            ui.label(format!(
                                "{} parts  ·  {:.3} kg  ·  viability {:.3}",
                                g.traits.parts, g.traits.mass, g.viability
                            ));
                            if g.exploit {
                                ui.colored_label(GOLD, "Energy audit: exploit suspect");
                            }
                            ui.separator();
                            radar(ui, &g.traits, world, view.selected);
                            ui.collapsing("All measured traits", |ui| {
                                for (i, v) in g.traits.values().iter().enumerate() {
                                    ui.label(format!("{}: {:.4}", TRAITS[i], v));
                                }
                            });
                            ui.collapsing("Morphology graph", |ui| {
                                for (i, b) in g.genome.bones.iter().enumerate() {
                                    ui.monospace(format!(
                                        "{i}: {:?} · recur {} · {} muscles",
                                        b.shape,
                                        b.recursion_limit,
                                        b.muscles.len()
                                    ));
                                    for e in &b.edges {
                                        ui.small(format!(
                                            "    → {}  {:?}  mirror:{}",
                                            e.child, e.kind, e.reflect
                                        ));
                                    }
                                }
                            });
                        }
                        if let Some(a) = world
                            .agents
                            .iter()
                            .find(|a| Some(a.id) == view.selected && a.genome == view.genome)
                        {
                            ui.separator();
                            ui.label(format!("Age {} ticks  ·  offspring {}", a.age, a.offspring));
                            ui.add(egui::ProgressBar::new(a.health).text("Health"));
                            ui.add(egui::ProgressBar::new(a.growth).text("Growth"));
                            ui.label(format!(
                                "Energy {} J · injury {:.1}%",
                                data::number(a.energy as f64 / 1000.0),
                                a.injury * 100.0
                            ));
                            ui.horizontal_wrapped(|ui| {
                                ui.label("Parents:");
                                for id in &a.parents {
                                    if ui.link(format!("#{id}")).clicked() {
                                        focus(&mut view, *id, &feed);
                                    }
                                }
                            });
                            ui.collapsing("Ethology outputs", |ui| {
                                for (i, v) in a.outputs.iter().enumerate() {
                                    ui.add(egui::ProgressBar::new(v.clamp(0.0, 1.0)).text(
                                        format!("{}  {v:.2}", ACTIONS.get(i).unwrap_or(&"Neuron")),
                                    ));
                                }
                            });
                        }
                        if let Some(life) = view.selected.and_then(|id| world.history.get(&id)) {
                            ui.collapsing("Lineage", |ui| {
                                for parent in &life.parents {
                                    if ui.link(format!("Parent #{parent}")).clicked() { focus(&mut view, *parent, &feed); }
                                }
                                for child in world.history.values().filter(|child| child.parents.contains(&life.id)) {
                                    if ui.link(format!("Offspring #{} · born {}", child.id, child.born)).clicked() { focus(&mut view, child.id, &feed); }
                                }
                            });
                        }
                        if view.live_preview { ui.collapsing("Live motor neurons · local physics arena", |ui| {
                            if let Some(creature) = &inspector.creature {
                                for (i, pool) in creature.brains.iter().enumerate() {
                                    ui.label(format!("Bone {i}"));
                                    for (n, v) in pool.values.iter().enumerate() {
                                        ui.add(
                                            egui::ProgressBar::new(v.clamp(0.0, 1.0))
                                                .text(format!("n{n}  {v:.3}")),
                                        );
                                    }
                                }
                            }
                        }); }
                        ui.separator();
                        ui.horizontal_wrapped(|ui| {
                            ui.selectable_value(&mut view.live_preview, true, "Live physics");
                            for trial in [TrialKind::Sprint, TrialKind::Strike] {
                                if ui.selectable_label(!view.live_preview && view.trial == trial, format!("Recorded {trial:?}")).clicked() {
                                    view.live_preview = false;
                                    view.trial = trial;
                                    view.play_time = 0.0;
                                    view.replay_frame = 0;
                                    let key = (view.genome, trial);
                                    request_replay(&mut view, key, &feed);
                                }
                            }
                        });
                        if ui
                            .button(if view.playing {
                                "Pause preview"
                            } else {
                                "Play preview"
                            })
                            .clicked()
                        {
                            view.playing = !view.playing;
                        }
                        if let Some(replay) = view.replays.get(&(view.genome, view.trial)).cloned() {
                            let end = replay.frames.len().saturating_sub(1);
                            if ui
                                .add(
                                    egui::Slider::new(&mut view.replay_frame, 0..=end)
                                        .text("Recorded sprint frame"),
                                )
                                .changed()
                            {
                                view.playing = false;
                                view.live_preview = false;
                                view.play_time = view.replay_frame as f32 * replay.dt;
                            }
                            replay_picture(ui, &replay, view.replay_frame, world.genomes.get(&view.genome), &world.config.genome);
                        } else if !view.live_preview {
                            ui.label(if view.unavailable.contains(&(view.genome, view.trial)) { "This trial has no recorded replay (it may have been skipped by the stand gate)." } else { "Loading recorded poses…" });
                        }
                    });
                });
        }
        Tab::Archive => {
            egui::CentralPanel::default().show(ctx, |ui| archive(ui, &mut view, &feed));
        }
        Tab::Phylogeny => {
            egui::CentralPanel::default().show(ctx, |ui| phylogeny(ui, &mut view, &feed));
        }
        Tab::Charts => {
            egui::CentralPanel::default().show(ctx, |ui| charts(ui, &snapshot.samples));
        }
        Tab::Highlights => {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Highlights & energy audits");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (tick, event) in snapshot
                        .events
                        .iter()
                        .rev()
                        .filter(|(_, e)| data::notable(e))
                    {
                        ui.horizontal(|ui| {
                            if ui
                                .link(format!("{tick:>9}  {}", data::event_title(event)))
                                .clicked()
                            {
                                jump_to_event(&mut view, *tick, event, &feed);
                            }
                        });
                    }
                });
            });
        }
    }
}
fn jump_to_event(view: &mut View, tick: u64, event: &bestiary_world::EcologyEvent, feed: &Feed) {
    view.highlight_tick = tick;
    if let Some((selected, hash)) = data::event_subject(&view.snapshot.world, event) {
        view.selected = selected;
        select_genome(view, hash, feed);
        view.tab = Tab::Inspector;
        view.radius = 5.0;
    }
}
#[derive(Debug, PartialEq, Eq)]
enum ClipReadiness {
    Ready { static_bodies: usize },
    Loading,
    NoRecording,
    LivePreview,
    UnsupportedPanel,
}
impl ClipReadiness {
    fn message(&self) -> Option<String> {
        match self {
            Self::Ready { static_bodies: 0 } | Self::UnsupportedPanel => None,
            Self::Ready { static_bodies } => Some(format!(
                "{static_bodies} bodies have no Sprint recording and will remain static."
            )),
            Self::Loading => Some("Loading recorded poses before camera export…".into()),
            Self::NoRecording => Some("No recorded poses are available for camera export.".into()),
            Self::LivePreview => Some("Select a recorded replay to export this body.".into()),
        }
    }
}
fn clip_readiness(view: &View) -> ClipReadiness {
    let required: Vec<_> = match view.tab {
        Tab::Inspector if view.live_preview => return ClipReadiness::LivePreview,
        Tab::Inspector => vec![(view.genome, view.trial)],
        Tab::World => {
            let world = &view.snapshot.world;
            // Follow mode updates the camera after this UI, so use the same
            // current subject position when choosing the bodies to export.
            let target = if view.mode == CameraMode::Follow {
                world
                    .agents
                    .iter()
                    .find(|a| Some(a.id) == view.selected)
                    .map(|a| Vec3::new(a.position[0], 0.0, a.position[1]))
                    .unwrap_or(view.target)
            } else {
                view.target
            };
            let near = scene::nearest_bodies(world, target, 20);
            world
                .agents
                .iter()
                .filter(|a| near.contains(&a.id))
                .map(|a| (a.genome, TrialKind::Sprint))
                .collect()
        }
        _ => return ClipReadiness::UnsupportedPanel,
    };
    if required
        .iter()
        .any(|key| !view.replays.contains_key(key) && !view.unavailable.contains(key))
    {
        return ClipReadiness::Loading;
    }
    let recorded = required
        .iter()
        .filter(|key| view.replays.get(key).is_some_and(|r| !r.frames.is_empty()))
        .count();
    if recorded == 0 {
        ClipReadiness::NoRecording
    } else {
        ClipReadiness::Ready {
            static_bodies: required.len() - recorded,
        }
    }
}
const ACTIONS: [&str; 13] = [
    "Move X", "Move Z", "Effort", "Forage", "Pursue", "Flee", "Attack", "Mate", "Rest", "Emit 0",
    "Emit 1", "Emit 2", "Emit 3",
];
const TRAITS: [&str; 18] = [
    "Sprint m/s",
    "Acceleration",
    "Energy J/m",
    "Turn rad/s",
    "Rough m/s",
    "Slope rad",
    "Swim m/s",
    "Strike impulse",
    "Carry kg",
    "Reach m",
    "Volume",
    "Mass kg",
    "Parts",
    "Limbs",
    "Symmetry",
    "Gait Hz",
    "Posture watts",
    "Muscle area",
];
fn start_clip(view: &mut View) {
    let readiness = clip_readiness(view);
    if view.clip.is_some() || !matches!(readiness, ClipReadiness::Ready { .. }) {
        view.error = readiness
            .message()
            .unwrap_or_else(|| "Camera export is unavailable in this panel.".into());
        return;
    }
    let schedule = match playback::ClipSchedule::new(view.clip_start, view.clip_seconds, 30) {
        Ok(schedule) => schedule,
        Err(error) => {
            view.error = error.into();
            return;
        }
    };
    let directory =
        PathBuf::from("exports").join(format!("clip-{}-{}", view.snapshot.world.tick, view.frame));
    if let Err(error) = export::start(view, directory, schedule, false) {
        view.error = error;
    }
}
fn radar(
    ui: &mut egui::Ui,
    traits: &bestiary_sim::combine::Traits,
    world: &bestiary_world::World,
    selected: Option<u64>,
) {
    let selected_species = world
        .agents
        .iter()
        .find(|a| Some(a.id) == selected)
        .map(|a| a.species);
    let values = descriptors(traits);
    let global: [f32; 7] = std::array::from_fn(|i| {
        data::median(
            world
                .agents
                .iter()
                .map(|a| descriptors(&world.genomes[&a.genome].traits)[i])
                .collect(),
        )
    });
    let species: [f32; 7] = std::array::from_fn(|i| {
        data::median(
            world
                .agents
                .iter()
                .filter(|a| Some(a.species) == selected_species)
                .map(|a| descriptors(&world.genomes[&a.genome].traits)[i])
                .collect(),
        )
    });
    let (response, painter) = ui.allocate_painter(
        egui::vec2(ui.available_width(), 220.0),
        egui::Sense::hover(),
    );
    let center = response.rect.center();
    let radius = 76.0;
    let point = |i: usize, r: f32| {
        center
            + egui::vec2(
                (i as f32 * std::f32::consts::TAU / 7.0 - std::f32::consts::FRAC_PI_2).cos(),
                (i as f32 * std::f32::consts::TAU / 7.0 - std::f32::consts::FRAC_PI_2).sin(),
            ) * r
    };
    for (i, label) in AXES.iter().enumerate() {
        painter.line_segment(
            [center, point(i, radius)],
            egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
        );
        painter.text(
            point(i, radius + 22.0),
            egui::Align2::CENTER_CENTER,
            *label,
            egui::FontId::proportional(10.0),
            egui::Color32::GRAY,
        );
    }
    for (series, color) in [
        (&global, egui::Color32::GRAY),
        (&species, GOLD),
        (&values, TEAL),
    ] {
        let mut points: Vec<_> = (0..7)
            .map(|i| {
                point(
                    i,
                    radius * series[i] / values[i].max(global[i]).max(species[i]).max(0.001),
                )
            })
            .collect();
        points.push(points[0]);
        painter.add(egui::Shape::line(points, egui::Stroke::new(2.0, color)));
    }
    ui.small("Green: creature · gold: species median · gray: world median");
}
fn replay_picture(
    ui: &mut egui::Ui,
    replay: &Replay,
    index: usize,
    entry: Option<&bestiary_world::LineageGenome>,
    limits: &bestiary_core::genome::Limits,
) {
    let (response, painter) = ui.allocate_painter(egui::vec2(300.0, 170.0), egui::Sense::hover());
    painter.rect_filled(response.rect, 4.0, egui::Color32::from_rgb(9, 16, 21));
    if let (Some(poses), Some(entry)) = (replay.frames.get(index), entry) {
        if let Ok(body) = entry.genome.expand(limits) {
            let radius = body
                .parts
                .iter()
                .map(|p| {
                    Vec3::from_array(p.pose.translation.vector.into()).length()
                        + Vec3::from_array(p.size).length()
                })
                .fold(1.0f32, f32::max);
            let scale = 80.0 / radius;
            let root = poses
                .first()
                .map(|p| Vec3::new(p.translation[0], 0.0, p.translation[2]))
                .unwrap_or_default();
            let project = |v: Vec3| {
                response.rect.center()
                    + egui::vec2(
                        (v.x - v.z * 0.55) * scale,
                        35.0 + (-v.y + v.z * 0.3) * scale,
                    )
            };
            let point = |part: usize, local: Vec3| {
                poses.get(part).map(|p| {
                    project(
                        Vec3::from_array(p.translation) + Quat::from_array(p.rotation) * local
                            - root,
                    )
                })
            };
            for (i, part) in body.parts.iter().enumerate() {
                let rgb = entry.genome.bones[part.gene].color;
                let color = egui::Color32::from_rgb(
                    (rgb[0] * 255.0) as u8,
                    (rgb[1] * 255.0) as u8,
                    (rgb[2] * 255.0) as u8,
                );
                if part.shape == bestiary_core::genome::Shape::Capsule {
                    let radius = part.size[0].min(part.size[2]) * 0.5;
                    let half = (part.size[1] * 0.5 - radius).max(0.0);
                    if let (Some(a), Some(b)) =
                        (point(i, Vec3::Y * half), point(i, -Vec3::Y * half))
                    {
                        painter.line_segment(
                            [a, b],
                            egui::Stroke::new((radius * scale * 2.0).max(1.0), color),
                        );
                        painter.circle_filled(a, radius * scale, color);
                        painter.circle_filled(b, radius * scale, color);
                    }
                } else {
                    for corner in 0..8 {
                        let local = |n: usize| {
                            Vec3::new(
                                if n & 1 == 0 { -1.0 } else { 1.0 },
                                if n & 2 == 0 { -1.0 } else { 1.0 },
                                if n & 4 == 0 { -1.0 } else { 1.0 },
                            ) * Vec3::from_array(part.size)
                                * 0.5
                        };
                        for axis in [1, 2, 4] {
                            if corner & axis == 0 {
                                if let (Some(a), Some(b)) =
                                    (point(i, local(corner)), point(i, local(corner | axis)))
                                {
                                    painter.line_segment([a, b], egui::Stroke::new(1.5, color));
                                }
                            }
                        }
                    }
                }
            }
            if scene::strike_target_pose(replay, index, body.parts.len()).is_some() {
                let half = Vec3::from_array(bestiary_sim::combine::STRIKE_TARGET_SIZE) * 0.5;
                let corner = |n: usize| {
                    Vec3::new(
                        if n & 1 == 0 { -half.x } else { half.x },
                        if n & 2 == 0 { -half.y } else { half.y },
                        if n & 4 == 0 { -half.z } else { half.z },
                    )
                };
                for vertex in 0..8 {
                    for axis in [1, 2, 4] {
                        if vertex & axis == 0 {
                            if let (Some(a), Some(b)) = (
                                point(body.parts.len(), corner(vertex)),
                                point(body.parts.len(), corner(vertex | axis)),
                            ) {
                                painter.line_segment([a, b], egui::Stroke::new(2.0, GOLD));
                            }
                        }
                    }
                }
            }
            for m in &body.muscles {
                if let (Some(a), Some(b)) = (
                    point(m.origin_part, Vec3::from_array(m.origin)),
                    point(m.insertion_part, Vec3::from_array(m.insertion)),
                ) {
                    painter.line_segment([a, b], egui::Stroke::new(1.0, egui::Color32::LIGHT_RED));
                }
            }
        }
    }
    painter.text(
        response.rect.left_top() + egui::vec2(8.0, 8.0),
        egui::Align2::LEFT_TOP,
        format!("{:?} · {:.2} s", replay.kind, index as f32 * replay.dt),
        egui::FontId::proportional(11.0),
        egui::Color32::GRAY,
    );
}

fn archive(ui: &mut egui::Ui, view: &mut View, feed: &Feed) {
    let snapshot = view.snapshot.clone();
    let world = &snapshot.world;
    ui.heading("The body-plan archive");
    ui.label("Each cell retains the highest-viability body discovered in its descriptor range.");
    ui.horizontal(|ui| {
        for dimension in 0..2 {
            egui::ComboBox::from_id_salt(format!("axis-{dimension}"))
                .selected_text(AXES[view.axes[dimension]])
                .show_ui(ui, |ui| {
                    for (i, name) in AXES.iter().enumerate() {
                        ui.selectable_value(&mut view.axes[dimension], i, *name);
                    }
                });
        }
    });
    ui.horizontal_wrapped(|ui| {
        for (i, name) in AXES.iter().enumerate() {
            if !view.axes.contains(&i) {
                let end = world.config.archive.thresholds[i].len() as u8;
                ui.add(egui::Slider::new(&mut view.slices[i], 0..=end).text(*name));
            }
        }
    });
    if view.axes[0] == view.axes[1] {
        ui.colored_label(GOLD, "Choose two different descriptor axes.");
        return;
    }
    let nx = world.config.archive.thresholds[view.axes[0]].len() + 1;
    let ny = world.config.archive.thresholds[view.axes[1]].len() + 1;
    for y in (0..ny).rev() {
        ui.horizontal(|ui| {
            ui.label(format!("{} bin {y}", AXES[view.axes[1]]));
            for x in 0..nx {
                let mut key = view.slices;
                key[view.axes[0]] = x as u8;
                key[view.axes[1]] = y as u8;
                if let Some(elite) = world.archive.cells.get(&key) {
                    let label = format!(
                        "{} parts\n{:.2} kg\nviability {:.3}",
                        elite.traits.parts, elite.traits.mass, elite.viability
                    );
                    request_replay(view, (elite.genome, TrialKind::Sprint), feed);
                    let response = ui
                        .vertical(|ui| {
                            let response = ui.add_sized(
                                [300.0, 55.0],
                                egui::Button::new(label).fill(egui::Color32::from_rgb(25, 73, 65)),
                            );
                            if let Some(replay) =
                                view.replays.get(&(elite.genome, TrialKind::Sprint))
                            {
                                let frame = if response.hovered() {
                                    playback::frame_index(
                                        view.play_time,
                                        replay.dt,
                                        replay.frames.len(),
                                    )
                                } else {
                                    0
                                };
                                replay_picture(
                                    ui,
                                    replay,
                                    frame,
                                    world.genomes.get(&elite.genome),
                                    &world.config.genome,
                                );
                            } else {
                                ui.add_sized(
                                    [300.0, 170.0],
                                    egui::Label::new("Loading body replay…"),
                                );
                            }
                            response
                        })
                        .inner;
                    if response.clicked() {
                        select_genome(view, elite.genome, feed);
                        view.tab = Tab::Inspector;
                        view.radius = 5.0;
                    }
                } else {
                    ui.add_sized(
                        [300.0, 233.0],
                        egui::Button::new("Undiscovered").sense(egui::Sense::hover()),
                    );
                }
            }
        });
    }
    ui.label(format!("Horizontal: {}", AXES[view.axes[0]]));
    ui.separator();
    ui.label("All occupied cells");
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (key, e) in &world.archive.cells {
            if ui
                .link(format!(
                    "{key:?}  ·  {} parts  ·  {:.3} kg  ·  viability {:.3}",
                    e.traits.parts, e.traits.mass, e.viability
                ))
                .clicked()
            {
                view.slices = *key;
                select_genome(view, e.genome, feed);
            }
        }
    });
}
fn phylogeny(ui: &mut egui::Ui, view: &mut View, feed: &Feed) {
    let snapshot = view.snapshot.clone();
    let w = &snapshot.world;
    ui.heading("Phylogeny");
    ui.label("Time runs left to right. Branch width shows the last sampled population at or before the selected tick.");
    let max_tick = w.tick.max(1);
    ui.add(egui::Slider::new(&mut view.highlight_tick, 0..=max_tick).text("Population at tick"));
    egui::ScrollArea::both().show(ui, |ui| {
        let width = ui.available_width().max(700.0);
        let height = (w.registry.species.len() as f32 * 34.0 + 60.0).max(200.0);
        let (response, painter) =
            ui.allocate_painter(egui::vec2(width, height), egui::Sense::click());
        let rows: BTreeMap<_, _> = w
            .registry
            .species
            .keys()
            .enumerate()
            .map(|(i, id)| (*id, i))
            .collect();
        let point = |tick: u64, row: usize| {
            response.rect.left_top()
                + egui::vec2(
                    180.0 + tick as f32 / max_tick as f32 * (width - 220.0),
                    35.0 + row as f32 * 34.0,
                )
        };
        for s in w.registry.species.values() {
            let row = rows[&s.id];
            let start = point(s.born, row);
            let end = point(s.extinct.unwrap_or(max_tick), row);
            let population =
                data::species_population_at(&snapshot.samples, s.id, view.highlight_tick);
            let color = crate::scene::species_color(s.id);
            let color = egui::Color32::from_rgb(
                (color[0] * 255.0) as u8,
                (color[1] * 255.0) as u8,
                (color[2] * 255.0) as u8,
            );
            if let Some(parent) = s
                .parent
                .and_then(|id| rows.get(&id))
                .filter(|_| ui.clip_rect().contains(start))
            {
                painter.line_segment(
                    [point(s.born, *parent), start],
                    egui::Stroke::new(1.0, egui::Color32::GRAY),
                );
            }
            painter.line_segment(
                [start, end],
                egui::Stroke::new(2.0 + (population as f32).sqrt(), color),
            );
            painter.text(
                egui::pos2(response.rect.left() + 5.0, start.y),
                egui::Align2::LEFT_CENTER,
                &s.name,
                egui::FontId::proportional(12.0),
                color,
            );
            let click = ui.interact(
                egui::Rect::from_min_max(
                    egui::pos2(response.rect.left(), start.y - 15.0),
                    egui::pos2(response.rect.right(), start.y + 15.0),
                ),
                ui.id().with(s.id),
                egui::Sense::click(),
            );
            if click.clicked() {
                select_genome(view, s.founder, feed);
                view.tab = Tab::Inspector;
                view.radius = 5.0;
            }
            click.on_hover_text(format!(
                "Born {} · parent {:?} · population {}",
                s.born, s.parent, population
            ));
        }
    });
}
type ChartSeries<'a> = (&'a str, Vec<(u64, f32)>, egui::Color32);
fn chart(ui: &mut egui::Ui, label: &str, series: &[ChartSeries<'_>]) {
    ui.label(egui::RichText::new(label).strong());
    let (response, painter) = ui.allocate_painter(
        egui::vec2(ui.available_width(), 150.0),
        egui::Sense::hover(),
    );
    let rect = response.rect.shrink2(egui::vec2(8.0, 16.0));
    let max_tick = series
        .iter()
        .flat_map(|(_, p, _)| p.iter().map(|p| p.0))
        .max()
        .unwrap_or(1)
        .max(1);
    let max = series
        .iter()
        .flat_map(|(_, p, _)| p.iter().map(|p| p.1))
        .fold(0.001f32, f32::max);
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
    );
    for (name, points, color) in series {
        let points: Vec<_> = data::bins(points, 500)
            .iter()
            .map(|(tick, v)| {
                egui::pos2(
                    rect.left() + *tick as f32 / max_tick as f32 * rect.width(),
                    rect.bottom() - v / max * rect.height(),
                )
            })
            .collect();
        if points.len() > 1 {
            painter.add(egui::Shape::line(points, egui::Stroke::new(1.5, *color)));
        }
        ui.colored_label(*color, *name);
    }
    painter.text(
        rect.left_top(),
        egui::Align2::LEFT_TOP,
        format!("max {}", data::number(max as f64)),
        egui::FontId::proportional(10.0),
        egui::Color32::GRAY,
    );
}
fn charts(ui: &mut egui::Ui, samples: &[bestiary_archive::store::TimeSample]) {
    ui.heading("World history");
    if samples.is_empty() {
        ui.label("No stored time-series samples yet.");
        return;
    }
    egui::ScrollArea::vertical().show(ui, |ui| {
        chart(
            ui,
            "Population",
            &[(
                "Creatures",
                samples
                    .iter()
                    .map(|s| (s.tick, s.population as f32))
                    .collect(),
                TEAL,
            )],
        );
        chart(
            ui,
            "Living species",
            &[(
                "Species",
                samples.iter().map(|s| (s.tick, s.species as f32)).collect(),
                GOLD,
            )],
        );
        chart(
            ui,
            "Biomass by diet",
            &[
                (
                    "Herbivores",
                    samples
                        .iter()
                        .map(|s| (s.tick, s.trophic_biomass[0] as f32))
                        .collect(),
                    TEAL,
                ),
                (
                    "Omnivores",
                    samples
                        .iter()
                        .map(|s| (s.tick, s.trophic_biomass[1] as f32))
                        .collect(),
                    GOLD,
                ),
                (
                    "Carnivores",
                    samples
                        .iter()
                        .map(|s| (s.tick, s.trophic_biomass[2] as f32))
                        .collect(),
                    egui::Color32::LIGHT_RED,
                ),
            ],
        );
        chart(
            ui,
            "Archive coverage",
            &[(
                "Fraction of configured cells",
                samples.iter().map(|s| (s.tick, s.coverage)).collect(),
                TEAL,
            )],
        );
        for index in [0, 11, 12] {
            chart(
                ui,
                TRAITS[index],
                &[
                    (
                        "Mean",
                        samples
                            .iter()
                            .map(|s| (s.tick, s.mean_traits.get(index).copied().unwrap_or(0.0)))
                            .collect(),
                        TEAL,
                    ),
                    (
                        "Variance",
                        samples
                            .iter()
                            .map(|s| (s.tick, s.trait_variance.get(index).copied().unwrap_or(0.0)))
                            .collect(),
                        GOLD,
                    ),
                ],
            );
        }
        chart(
            ui,
            "Plant defense and herbivore resistance",
            &[
                (
                    "Plant toxicity",
                    samples.iter().map(|s| (s.tick, s.plant_toxicity)).collect(),
                    GOLD,
                ),
                (
                    "Immune investment",
                    samples
                        .iter()
                        .map(|s| (s.tick, s.herbivore_resistance))
                        .collect(),
                    TEAL,
                ),
            ],
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bestiary_world::{EcologyEvent, World};
    use std::sync::{mpsc, Mutex};

    fn view(world: World) -> View {
        let (selected, genome) = data::initial_selection(&world).unwrap();
        View {
            snapshot: Arc::new(Snapshot {
                revision: world.state_hash(),
                world,
                samples: vec![],
                events: vec![],
                live: false,
                paused: false,
                physics_revision: Some(bestiary_sim::SIMULATION_REVISION),
            }),
            tab: Tab::Highlights,
            overlay: Overlay::Biomass,
            selected,
            genome,
            mode: CameraMode::Free,
            target: Vec3::ZERO,
            radius: 30.0,
            yaw: 0.6,
            pitch: 0.7,
            dirty: false,
            paused: false,
            speed: 1.0,
            timer: Timer::from_seconds(1.0, TimerMode::Repeating),
            pending_refresh: false,
            replays: BTreeMap::new(),
            pending: BTreeSet::new(),
            unavailable: BTreeSet::new(),
            live_preview: false,
            anatomy: true,
            trial: TrialKind::Sprint,
            error: String::new(),
            axes: [0, 2],
            slices: [0; 7],
            replay_frame: 30,
            playing: false,
            play_time: 1.0,
            world_play_time: 1.0,
            highlight_tick: 0,
            clip: None,
            clip_start: 0.0,
            clip_seconds: 1.0,
            smoke: None,
            benchmark: false,
            frame: 0,
            benchmark_start: None,
            benchmark_frames: 0,
            benchmark_visible: [0; 2],
            canvas_rect: [0.0, 0.0, 1440.0, 960.0],
            canvas_scale: 1.0,
        }
    }

    fn two_genome_world() -> World {
        let mut world = World::new(42, bestiary_sim::config::Config::bundled(), 2).unwrap();
        let agent = &mut world.agents[0];
        let mut lineage = world.genomes[&agent.genome].clone();
        lineage.genome.bones[0].color = [0.1, 0.2, 0.3];
        agent.genome = lineage.genome.hash();
        world.history.get_mut(&agent.id).unwrap().genome = agent.genome;
        world.genomes.insert(agent.genome, lineage);
        world
    }

    #[test]
    fn manual_highlight_jump_opens_recorded_subject_and_never_invents_a_global_subject() {
        let world = two_genome_world();
        let attacker = world.agents[0].id;
        let hash = world.agents[0].genome;
        let victim = world.agents[1].id;
        let victim_hash = world.agents[1].genome;
        let (tx, _jobs) = mpsc::channel();
        let (_messages, rx) = mpsc::channel();
        let feed = Feed {
            tx,
            rx: Mutex::new(rx),
        };
        let events = [
            EcologyEvent::FirstPredation { attacker, victim },
            EcologyEvent::FirstPheromoneGuidedHunt {
                attacker,
                victim,
                decision_tick: 2,
                pheromone_channels: 1,
            },
            EcologyEvent::FirstFlyerAdjacentGait {
                genome: hash,
                cycles: 2,
                max_clearance: 0.2,
            },
            EcologyEvent::RecordLifespan {
                creature: attacker,
                age: 50,
            },
        ];
        for living in [true, false] {
            let mut historical = world.clone();
            if !living {
                historical.agents.retain(|a| a.id != attacker);
            }
            for event in &events {
                let mut view = view(historical.clone());
                view.selected = Some(victim);
                view.genome = victim_hash;
                jump_to_event(&mut view, 50, event, &feed);
                assert_eq!(view.highlight_tick, 50);
                assert_eq!(view.tab, Tab::Inspector);
                assert_eq!(view.genome, hash);
                assert!(view.snapshot.world.genomes.contains_key(&view.genome));
                assert_eq!(view.selected, living.then_some(attacker));
                assert_eq!(view.play_time, 0.0);
                assert_eq!(view.replay_frame, 0);
                assert_eq!(view.radius, 5.0);
                assert!(view.dirty);
            }
        }
        for event in [
            EcologyEvent::MassExtinction,
            EcologyEvent::FirstPredation {
                attacker: 999,
                victim,
            },
        ] {
            let mut view = view(world.clone());
            view.selected = Some(victim);
            view.genome = victim_hash;
            jump_to_event(&mut view, 80, &event, &feed);
            assert_eq!(view.highlight_tick, 80);
            assert_eq!(view.tab, Tab::Highlights);
            assert_eq!(view.selected, Some(victim));
            assert_eq!(view.genome, victim_hash);
            assert!(!view.dirty);
        }
    }

    #[test]
    fn clip_readiness_waits_for_scene_recordings_and_labels_missing_static_bodies() {
        let world = two_genome_world();
        let first = (world.agents[0].genome, TrialKind::Sprint);
        let second = (world.agents[1].genome, TrialKind::Sprint);
        let mut view = view(world);
        view.tab = Tab::World;
        view.pending.insert(first);
        assert_eq!(clip_readiness(&view), ClipReadiness::Loading);
        start_clip(&mut view);
        assert!(view.clip.is_none());
        assert!(view.error.contains("Loading recorded poses"));
        let replay = Replay {
            kind: TrialKind::Sprint,
            dt: 1.0 / 240.0,
            frames: vec![vec![Pose {
                translation: [0.0, 1.0, 0.0],
                rotation: Quat::IDENTITY.to_array(),
            }]],
        };
        view.pending.remove(&first);
        view.replays.insert(first, replay.clone());
        assert_eq!(clip_readiness(&view), ClipReadiness::Loading);
        view.unavailable.insert(second);
        let ready = ClipReadiness::Ready { static_bodies: 1 };
        assert_eq!(clip_readiness(&view), ready);
        assert!(ready
            .message()
            .unwrap()
            .contains("1 bodies have no Sprint recording"));
        // Unrelated replay requests do not block a fully resolved current scene.
        view.pending.insert(([9; 32], TrialKind::Strike));
        assert_eq!(clip_readiness(&view), ready);
        view.replays.insert(second, replay);
        assert_eq!(
            clip_readiness(&view),
            ClipReadiness::Ready { static_bodies: 0 }
        );
        view.replays.clear();
        view.unavailable.insert(first);
        assert_eq!(clip_readiness(&view), ClipReadiness::NoRecording);
        start_clip(&mut view);
        assert!(view.clip.is_none());
        assert!(view.error.contains("No recorded poses"));
        view.tab = Tab::Inspector;
        assert_eq!(clip_readiness(&view), ClipReadiness::NoRecording);
        view.live_preview = true;
        assert_eq!(clip_readiness(&view), ClipReadiness::LivePreview);
    }

    #[test]
    fn world_clip_preparation_advances_only_its_fixed_time_pose_clock() {
        let world = two_genome_world();
        for tab in [Tab::World, Tab::Inspector] {
            let mut view = view(world.clone());
            view.tab = tab;
            view.world_play_time = 17.0;
            view.clip = Some(Clip {
                dir: PathBuf::new(),
                schedule: playback::ClipSchedule::new(2.0, 0.1, 30).unwrap(),
                resume_playing: false,
                exit_after: false,
                first_frame_at: 0,
                metadata: serde_json::Value::Null,
            });
            let mut app = App::new();
            app.insert_resource(view)
                .add_systems(Update, prepare_clip_frame);
            for index in 0..3 {
                app.update();
                let expected = 2.0 + index as f32 / 30.0;
                {
                    let mut view = app.world_mut().resource_mut::<View>();
                    assert_eq!(view.play_time, expected);
                    assert_eq!(
                        view.world_play_time,
                        if tab == Tab::World { expected } else { 17.0 }
                    );
                    let frame = view
                        .clip
                        .as_mut()
                        .unwrap()
                        .schedule
                        .begin_capture()
                        .unwrap();
                    assert_eq!(frame.index, index);
                }
                // GPU completion latency may add updates, but never pose time.
                for _ in 0..4 {
                    app.update();
                }
                let mut view = app.world_mut().resource_mut::<View>();
                assert_eq!(view.play_time, expected);
                assert_eq!(
                    view.world_play_time,
                    if tab == Tab::World { expected } else { 17.0 }
                );
                assert!(view.clip.as_mut().unwrap().schedule.complete(index));
            }
        }
    }
}
