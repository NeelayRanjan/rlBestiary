//! The Training tab: setup, fitness chart, generation scrubber and actions.
use super::*;
use crate::training::{self, Command, Source};
use bestiary_core::genome::Genome;
use bestiary_sim::training::Task;
use bevy_egui::egui;

/// Distance between neighbouring training lanes, in metres.
pub(crate) const LANE_SPACING: f32 = 1.6;
const GHOST: [f32; 3] = [0.42, 0.46, 0.48];

/// Lane offsets are centred on the middle lane so the camera frames the row.
pub(crate) fn lane_offset(slot: usize, count: usize) -> Vec3 {
    Vec3::new(
        0.0,
        0.0,
        (slot as f32 - count.saturating_sub(1) as f32 / 2.0) * LANE_SPACING,
    )
}
pub(crate) fn ghost_color(champion: bool) -> Option<[f32; 3]> {
    (!champion).then_some(GHOST)
}
/// Chart points for best and median fitness; zero sits on the bottom edge.
pub(crate) fn series(
    h: &[training::Summary],
    rect: egui::Rect,
) -> (Vec<egui::Pos2>, Vec<egui::Pos2>) {
    if h.is_empty() {
        return (vec![], vec![]);
    }
    let top = h.iter().map(|s| s.best).fold(f32::MIN, f32::max).max(1e-6);
    let span = (h.len() - 1).max(1) as f32;
    let point = |i: usize, v: f32| {
        egui::pos2(
            rect.left() + rect.width() * i as f32 / span,
            rect.bottom() - rect.height() * (v.max(0.0) / top),
        )
    };
    (
        h.iter()
            .enumerate()
            .map(|(i, s)| point(i, s.best))
            .collect(),
        h.iter()
            .enumerate()
            .map(|(i, s)| point(i, s.median))
            .collect(),
    )
}
fn fitness_chart(ui: &mut egui::Ui, history: &[training::Summary]) {
    let (response, painter) = ui.allocate_painter(
        egui::vec2(ui.available_width(), 160.0),
        egui::Sense::hover(),
    );
    painter.rect_filled(response.rect, 4.0, egui::Color32::from_rgb(9, 16, 21));
    let (best, median) = series(history, response.rect.shrink(8.0));
    painter.add(egui::Shape::line(
        median,
        egui::Stroke::new(1.5, egui::Color32::GRAY),
    ));
    painter.add(egui::Shape::line(
        best,
        egui::Stroke::new(2.0, crate::ui::TEAL),
    ));
    ui.small("Teal: best of generation · gray: median");
}

enum Action {
    Open(Genome),
    Release(Genome),
}

pub fn panel(ctx: &egui::Context, view: &mut View, worker: &training::Worker, feed: &Feed) {
    let config = view.snapshot.world.config.clone();
    let zoo = bestiary_core::genome::zoo(&config.genome);
    let draft = view.designer.genome.clone();
    let selected = view
        .snapshot
        .world
        .genomes
        .get(&view.genome)
        .map(|g| g.genome.clone());
    let live = view.snapshot.live;
    let mut t = std::mem::take(&mut view.training);
    let mut redraw = false;
    let mut action = None;
    egui::SidePanel::right("training")
        .min_width(320.0)
        .show(ctx, |ui| {
            ui.heading("Training");
            ui.small("A population of copies tries the task in real physics; the best carry on, mutated, to the next generation.");
            ui.add_enabled_ui(!t.running && t.history.is_empty(), |ui| {
                egui::ComboBox::from_label("Body")
                    .selected_text(match t.source {
                        Source::Preset(i) => zoo[i].0.to_string(),
                        Source::Designer => "Designer draft".into(),
                        Source::Selected => "Selected creature".into(),
                    })
                    .show_ui(ui, |ui| {
                        for (i, (name, _)) in zoo.iter().enumerate() {
                            ui.selectable_value(&mut t.source, Source::Preset(i), *name);
                        }
                        ui.selectable_value(&mut t.source, Source::Designer, "Designer draft");
                        ui.selectable_value(&mut t.source, Source::Selected, "Selected creature");
                    });
                egui::ComboBox::from_label("Task")
                    .selected_text(t.task.name())
                    .show_ui(ui, |ui| {
                        for task in Task::ALL {
                            ui.selectable_value(&mut t.task, task, task.name());
                        }
                    });
                ui.add(egui::Slider::new(&mut t.settings.population, 2..=200).text("population"));
                ui.checkbox(&mut t.settings.body_mutation, "Allow body mutation");
            });
            ui.horizontal(|ui| {
                if !t.running && ui.button(if t.history.is_empty() { "Start" } else { "Resume" }).clicked() {
                    if t.history.is_empty() {
                        let source = match t.source {
                            Source::Preset(i) => Some(zoo[i].1.clone()),
                            Source::Designer => Some(draft.clone()),
                            Source::Selected => selected.clone(),
                        };
                        if let Some(source) = source {
                            t.reset();
                            let sent = worker.tx.send(Command::Start {
                                run: t.run,
                                source: Box::new(source),
                                task: t.task,
                                settings: Box::new(t.settings.clone()),
                                config: Box::new(config.clone()),
                                seed: t.seed,
                            });
                            t.running = sent.is_ok();
                            t.status = "Evaluating generation 0…".into();
                        } else {
                            t.status = "Select a living creature first".into();
                        }
                    } else {
                        t.running = worker.tx.send(Command::Resume).is_ok();
                    }
                }
                if t.running && ui.button("Pause").clicked() {
                    let _ = worker.tx.send(Command::Pause);
                    t.running = false;
                    t.status = "Paused after the current generation".into();
                }
                if ui.button("Reset").clicked() {
                    let _ = worker.tx.send(Command::Stop);
                    t.reset();
                    t.status.clear();
                    redraw = true;
                }
            });
            if let Some(last) = t.history.last() {
                ui.label(format!(
                    "Generation {} · best {:.3} · median {:.3}",
                    last.generation, last.best, last.median
                ));
            }
            if !t.status.is_empty() {
                ui.small(&t.status);
            }
            fitness_chart(ui, &t.history);
            if let Some(latest) = t.champions.len().checked_sub(1) {
                let mut shown = t.shown.map_or(latest, |n| n as usize);
                if ui
                    .add(egui::Slider::new(&mut shown, 0..=latest).text("generation"))
                    .changed()
                {
                    t.shown = (shown != latest).then_some(shown as u32);
                    t.time = 0.0;
                    redraw = true;
                }
                if ui
                    .checkbox(&mut t.show_all, "Show the whole latest generation")
                    .changed()
                {
                    redraw = true;
                }
                let champion = t.champions[shown].0.clone();
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Save genome").clicked() {
                        let dir = std::path::Path::new("exports/training");
                        let path = dir.join(format!("{}.genome", bestiary_core::hex(&champion.hash())));
                        t.status = std::fs::create_dir_all(dir)
                            .and_then(|_| std::fs::write(&path, champion.to_share()))
                            .map(|_| format!("Saved {}", path.display()))
                            .unwrap_or_else(|e| e.to_string());
                    }
                    if ui.button("Open in designer").clicked() {
                        action = Some(Action::Open(champion.clone()));
                    }
                    if ui
                        .add_enabled(live, egui::Button::new("Release into world"))
                        .clicked()
                    {
                        action = Some(Action::Release(champion.clone()));
                    }
                });
                if !live {
                    ui.small("Start this world's daemon to release creatures.");
                }
            }
        });
    match action {
        Some(Action::Open(genome)) => {
            view.designer = designer::Designer::new(genome, &config);
            view.tab = Tab::Designer;
            view.radius = 5.0;
            redraw = true;
        }
        Some(Action::Release(genome)) => {
            let command = bestiary_daemon::Request::Release {
                genome,
                position: view.designer.position,
                count: view.designer.population,
            };
            t.status = if feed.tx.send(Job::Release(command)).is_ok() {
                "Releasing founding population…"
            } else {
                "World connection unavailable"
            }
            .into();
        }
        None => {}
    }
    view.training = t;
    view.dirty |= redraw;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fitness_series_maps_generations_to_chart_space() {
        let h = vec![
            training::Summary {
                generation: 0,
                best: 0.0,
                median: 0.0,
            },
            training::Summary {
                generation: 1,
                best: 2.0,
                median: 1.0,
            },
        ];
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(100.0, 50.0));
        let (best, median) = series(&h, rect);
        assert_eq!(best, vec![egui::pos2(0.0, 50.0), egui::pos2(100.0, 0.0)]);
        assert_eq!(median[1], egui::pos2(100.0, 25.0));
        assert!(series(&[], rect).0.is_empty());
    }
    #[test]
    fn lane_offsets_are_distinct_and_centered() {
        let lanes: Vec<_> = (0..5).map(|i| lane_offset(i, 5)).collect();
        assert_eq!(lanes[2], Vec3::ZERO);
        assert!(lanes
            .windows(2)
            .all(|w| (w[1].z - w[0].z - LANE_SPACING).abs() < 1e-5));
    }
}
