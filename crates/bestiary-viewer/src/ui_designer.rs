//! Native editor controls. Drafts never enter the displayed world snapshot.
use crate::*;
use bestiary_core::genome::{JointKind, Sensor, Shape};
use bevy_egui::egui;

fn result(designer: &mut designer::Designer, result: Result<(), String>) {
    if let Err(error) = result {
        designer.status = error;
    }
}

fn vector(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut [f32; 3],
    range: std::ops::RangeInclusive<f32>,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        for (axis, value) in ["X", "Y", "Z"].into_iter().zip(value) {
            changed |= ui
                .add(
                    egui::DragValue::new(value)
                        .range(range.clone())
                        .speed(0.01)
                        .prefix(format!("{axis} ")),
                )
                .changed();
        }
    });
    changed
}

#[derive(Clone, Copy)]
struct SurfaceControl {
    face: usize,
    point: [f32; 3],
}
fn selected_surface_face(point: [f32; 3], previous: Option<SurfaceControl>) -> usize {
    if let Some(previous) = previous.filter(|previous| previous.point == point) {
        return previous.face;
    }
    let axis = point.iter().position(|v| v.abs() == 1.0).unwrap_or(0);
    axis * 2 + usize::from(point[axis] < 0.0)
}
fn surface(ui: &mut egui::Ui, label: &str, point: &mut [f32; 3]) -> bool {
    let before = *point;
    ui.push_id(label, |ui| {
        ui.label(label);
        let id = ui.id().with("selected_surface");
        let mut face = selected_surface_face(*point, ui.ctx().data_mut(|data| data.get_temp(id)));
        let original_face = face;
        egui::ComboBox::from_id_salt("face")
            .selected_text(["+X", "−X", "+Y", "−Y", "+Z", "−Z"][face])
            .show_ui(ui, |ui| {
                for (i, name) in ["+X", "−X", "+Y", "−Y", "+Z", "−Z"].iter().enumerate() {
                    ui.selectable_value(&mut face, i, *name);
                }
            });
        if face != original_face {
            point[face / 2] = if face % 2 == 0 { 1.0 } else { -1.0 };
        }
        for (i, value) in point.iter_mut().enumerate() {
            if i != face / 2 {
                ui.add(egui::Slider::new(value, -1.0..=1.0).text(["X", "Y", "Z"][i]));
            }
        }
        ui.ctx().data_mut(|data| {
            data.insert_temp(
                id,
                SurfaceControl {
                    face,
                    point: *point,
                },
            )
        });
    });
    before != *point
}

pub fn panel(ctx: &egui::Context, view: &mut View, feed: &Feed) {
    let config = view.snapshot.world.config.clone();
    let before = view.designer.genome.hash();
    egui::SidePanel::right("designer").default_width(420.0).min_width(340.0).show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Creature designer");
            ui.small("Drag colored attachment handles on the body. Shift-click two bone surfaces to draw a muscle. Right-drag to orbit.");
            ui.horizontal_wrapped(|ui| {
                egui::ComboBox::from_id_salt("starter_zoo").selected_text("Load a zoo preset").show_ui(ui, |ui| {
                    for (index, (name, _)) in bestiary_core::genome::zoo(&config.genome).iter().enumerate() {
                        if ui.button(*name).clicked() { let r = view.designer.preset(index, &config); result(&mut view.designer, r); }
                    }
                });
                if ui.button("Copy inspected creature").clicked() {
                    if let Some(g) = view.snapshot.world.genomes.get(&view.genome) {
                        view.designer.genome = g.genome.clone();
                        view.designer.changed(&config);
                    }
                }
            });
            ui.horizontal(|ui| {
                for (value, label) in [(true, "Anatomy"), (false, "Collision shapes")] {
                    if ui.selectable_value(&mut view.anatomy, value, label).changed() { view.dirty = true; }
                }
            });
            let mut edited = false;
            ui.separator();
            ui.collapsing("Bones and recursion", |ui| {
                egui::ComboBox::from_id_salt("bone").selected_text(format!("Bone gene {}", view.designer.bone)).show_ui(ui, |ui| {
                    for index in 0..view.designer.genome.bones.len() { ui.selectable_value(&mut view.designer.bone, index, format!("Bone {index}")); }
                });
                ui.horizontal(|ui| {
                    if ui.button("Add attached bone").clicked() { let r = view.designer.add_bone(view.designer.bone, &config); result(&mut view.designer, r.map(|_| ())); }
                    if ui.button("Remove bone").clicked() { let r = view.designer.remove_bone(view.designer.bone, &config); result(&mut view.designer, r); }
                });
                if let Some(bone) = view.designer.genome.bones.get_mut(view.designer.bone) {
                    ui.horizontal(|ui| { edited |= ui.selectable_value(&mut bone.shape, Shape::Box, "Box").changed(); edited |= ui.selectable_value(&mut bone.shape, Shape::Capsule, "Capsule").changed(); });
                    edited |= vector(ui, "Dimensions (m)", &mut bone.size, config.genome.min_size..=10.0);
                    edited |= ui.add(egui::DragValue::new(&mut bone.density).range(0.01..=100.0).speed(0.01).prefix("Density kg/m³ ")).changed();
                    edited |= ui.add(egui::Slider::new(&mut bone.recursion_limit, 1..=12).text("Recursion count")).changed();
                    edited |= ui.color_edit_button_rgb(&mut bone.color).changed();
                    ui.small("Repeated and reflected instances inherit this bone gene's edits.");
                }
            });
            ui.collapsing("Joints and attachment points", |ui| {
                let count = view.designer.genome.bones.len();
                let id = egui::Id::new("designer_joint_child");
                let mut child = ctx.data_mut(|data| data.get_temp::<usize>(id).unwrap_or(0)).min(count.saturating_sub(1));
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut child).range(0..=count.saturating_sub(1)).prefix("Child bone "));
                    if ui.button("Connect joint").clicked() { let r = view.designer.add_joint(view.designer.bone, child, &config); result(&mut view.designer, r.map(|_| ())); }
                });
                ctx.data_mut(|data| data.insert_temp(id, child));
                let bone = view.designer.bone;
                let edges = view.designer.genome.bones.get(bone).map_or(0, |b| b.edges.len());
                if edges == 0 { ui.label("This bone has no outgoing joints."); return; }
                view.designer.edge = view.designer.edge.min(edges - 1);
                ui.add(egui::Slider::new(&mut view.designer.edge, 0..=edges-1).text("Joint"));
                if ui.button("Remove selected joint").clicked() { let r = view.designer.remove_joint(bone, view.designer.edge, &config); result(&mut view.designer, r); return; }
                let edge = &mut view.designer.genome.bones[bone].edges[view.designer.edge];
                ui.horizontal_wrapped(|ui| { for kind in [JointKind::Fixed, JointKind::Hinge, JointKind::Universal, JointKind::Ball] { edited |= ui.selectable_value(&mut edge.kind, kind, format!("{kind:?}")).changed(); } });
                edited |= surface(ui, "Parent attachment", &mut edge.attachment);
                edited |= surface(ui, "Child attachment", &mut edge.child_attachment);
                edited |= vector(ui, "Orientation (radians)", &mut edge.orientation, -std::f32::consts::PI..=std::f32::consts::PI);
                for (axis, limits) in edge.limits.iter_mut().enumerate() {
                    ui.horizontal(|ui| { ui.label(format!("{} limits", ["X", "Y", "Z"][axis])); for v in limits { edited |= ui.add(egui::DragValue::new(v).range(-std::f32::consts::PI..=std::f32::consts::PI).speed(0.01)).changed(); } });
                }
                edited |= ui.add(egui::Slider::new(&mut edge.scale, 0.25..=2.0).text("Subtree scale")).changed();
                edited |= ui.checkbox(&mut edge.reflect, "Reflect subtree").changed();
                edited |= ui.checkbox(&mut edge.terminal_only, "Attach only at terminal recursion").changed();
            });
            ui.collapsing("Muscles", |ui| {
                ui.label("Shift-click origin and insertion on the 3D body to add a muscle.");
                if let Ok(body) = view.designer.genome.expand(&config.genome) {
                    let id = egui::Id::new("designer_muscle_parts");
                    let mut parts = ctx.data_mut(|data| data.get_temp::<[usize; 2]>(id).unwrap_or([0, 1]));
                    ui.horizontal(|ui| {
                        for (label, part) in ["From part ", "To part "].into_iter().zip(&mut parts) {
                            *part = (*part).min(body.parts.len().saturating_sub(1));
                            ui.add(egui::DragValue::new(part).range(0..=body.parts.len().saturating_sub(1)).prefix(label));
                        }
                        if ui.button("Connect muscle").clicked() { let r = view.designer.add_muscle(parts[0], parts[1], &config); result(&mut view.designer, r); }
                    });
                    ctx.data_mut(|data| data.insert_temp(id, parts));
                }
                let bone = view.designer.bone;
                let count = view.designer.genome.bones.get(bone).map_or(0, |b| b.muscles.len());
                if count == 0 { ui.label("No authored muscles on this bone; expansion supplies required joint actuation."); return; }
                view.designer.muscle = view.designer.muscle.min(count - 1);
                ui.add(egui::Slider::new(&mut view.designer.muscle, 0..=count-1).text("Muscle"));
                if ui.button("Remove selected muscle").clicked() { let r = view.designer.remove_muscle(bone, view.designer.muscle, &config); result(&mut view.designer, r); return; }
                let muscle = &mut view.designer.genome.bones[bone].muscles[view.designer.muscle];
                ui.label(format!("Route: {:?} ({} joints)", muscle.route, muscle.route.len()));
                edited |= surface(ui, "Origin", &mut muscle.origin);
                edited |= surface(ui, "Insertion", &mut muscle.insertion);
                edited |= ui.add(egui::Slider::new(&mut muscle.area, 0.001..=10.0).logarithmic(true).text("Cross-section m²")).changed();
                edited |= ui.add(egui::Slider::new(&mut muscle.properties.contraction_scale, 0.1..=2.0).text("Contraction scale")).changed();
                edited |= ui.add(egui::Slider::new(&mut muscle.properties.stiffness_scale, 0.1..=2.0).text("Passive stiffness scale")).changed();
                edited |= ui.add(egui::Slider::new(&mut muscle.properties.damping_scale, 0.1..=2.0).text("Damping scale")).changed();
                ui.small(format!("Maximum span: {} joints. Draw different endpoints to change span.", config.genome.max_span));
            });
            ui.collapsing("Sensors", |ui| {
                if let Some(bone) = view.designer.genome.bones.get_mut(view.designer.bone) {
                    let mut remove = None;
                    for (index, sensor) in bone.sensors.iter().enumerate() { ui.horizontal(|ui| { ui.label(format!("{sensor:?}")); if ui.small_button("Remove").clicked() { remove = Some(index); } }); }
                    if let Some(index) = remove { bone.sensors.remove(index); edited = true; }
                    egui::ComboBox::from_id_salt("new_sensor").selected_text("Attach sensor").show_ui(ui, |ui| {
                        let mut options = vec![Sensor::Contact, Sensor::ContactForce, Sensor::TerrainContact, Sensor::TerrainForce];
                        for axis in 0..3 { options.extend([Sensor::JointAngle(axis), Sensor::JointVelocity(axis), Sensor::Gravity(axis)]); }
                        for muscle in 0..bone.muscles.len() { options.extend([Sensor::MuscleLength(muscle), Sensor::MuscleVelocity(muscle)]); }
                        for sensor in options { if ui.button(format!("{sensor:?}")).clicked() { bone.sensors.push(sensor); edited = true; } }
                    });
                }
            });
            ui.collapsing("Life history and social genes", |ui| {
                let life = &mut view.designer.genome.life;
                edited |= ui.add(egui::Slider::new(&mut life.plant_efficiency, 0.0..=1.0).text("Plant diet (flesh = 1 − plant)")).changed();
                edited |= ui.add(egui::Slider::new(&mut life.offspring_fraction, 0.0..=1.0).text("Offspring size fraction")).changed();
                edited |= ui.add(egui::Slider::new(&mut life.clutch_size, 1..=16).text("Clutch size")).changed();
                edited |= ui.add(egui::DragValue::new(&mut life.maturity).range(1..=1000000).prefix("Maturity tick ")).changed();
                edited |= ui.add(egui::DragValue::new(&mut life.senescence).range(2..=10000000).prefix("Senescence tick ")).changed();
                edited |= ui.add(egui::Slider::new(&mut life.immune_investment, 0.0..=1.0).text("Immune investment")).changed();
                edited |= ui.add(egui::Slider::new(&mut life.pheromone_channel, 0..=3).text("Pheromone channel")).changed();
                edited |= ui.add(egui::Slider::new(&mut life.pheromone_strength, 0.0..=1.0).text("Pheromone strength")).changed();
                for (label, values) in [("Resistance", &mut life.resistance), ("Mate preference", &mut life.mate_weights), ("Species marker", &mut life.species_marker)] {
                    for (i, value) in values.iter_mut().enumerate() { edited |= ui.add(egui::Slider::new(value, if label == "Mate preference" { -1.0..=1.0 } else { 0.0..=1.0 }).text(format!("{label} {i}"))).changed(); }
                }
            });
            if edited { view.designer.changed(&config); }
            let report = view.designer.report(&config);
            ui.separator();
            ui.heading("Validity");
            ui.label(format!("{} expanded parts · {:.3} kg", report.parts, report.mass));
            ui.label(format!("Basal estimate {:.4} W · maximum holding {:.4} W", report.basal_watts, report.maximum_isometric_watts));
            ui.small("Measured posture and movement costs appear after the combine.");
            for message in &report.errors { ui.colored_label(egui::Color32::LIGHT_RED, message); }
            for message in &report.warnings { ui.colored_label(egui::Color32::GOLD, message); }
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(report.errors.is_empty() && !view.designer.pending, egui::Button::new("Run combine")).clicked() {
                    view.designer.pending = feed.tx.send(Job::Combine { generation: view.designer.generation, genome: Box::new(view.designer.genome.clone()), config: Box::new(config.clone()) }).is_ok();
                    view.designer.status = if view.designer.pending { "Running nine physics trials…" } else { "Combine worker unavailable" }.into();
                }
                if ui.selectable_label(!view.designer.preview, "Edit body").clicked() { view.designer.preview = false; view.dirty = true; }
                if ui.selectable_label(view.designer.preview && view.live_preview, "Live muscle test").clicked() { view.designer.preview = true; view.live_preview = true; view.playing = true; view.dirty = true; }
            });
            if let Some(evaluation) = &view.designer.evaluation {
                ui.label(format!("Viability {:.4} · energy audit {}", evaluation.viability, if evaluation.exploit_suspect { "FAILED" } else { "clear" }));
                ui.collapsing("Measured trait vector", |ui| { for (label, value) in crate::ui::TRAITS.iter().zip(evaluation.traits.values()) { ui.label(format!("{label}: {value:.5}")); } });
            }
            if let Some(replay) = view.replays.get(&(view.designer.genome.hash(), TrialKind::Sprint)) {
                if ui.button("Play measured sprint").clicked() { view.designer.preview = true; view.live_preview = false; view.trial = TrialKind::Sprint; view.play_time = 0.0; view.playing = true; view.dirty = true; }
                if ui.add(egui::Slider::new(&mut view.replay_frame, 0..=replay.frames.len().saturating_sub(1)).text("Sprint frame")).changed() { view.designer.preview = true; view.live_preview = false; view.playing = false; view.play_time = view.replay_frame as f32 * replay.dt; }
            }
            ui.separator();
            ui.heading("Release founding population");
            ui.add(egui::DragValue::new(&mut view.designer.population).range(1..=config.world.population_limit).prefix("Count "));
            let extent = config.world.grid as f32 * config.world.cell_metres;
            ui.horizontal(|ui| { for (label, value) in ["World X ", "World Z "].into_iter().zip(&mut view.designer.position) { ui.add(egui::DragValue::new(value).range(0.0..=extent).prefix(label)); } });
            let viable = view.designer.evaluation.as_ref().is_some_and(|e| e.genome_hash == view.designer.genome.hash() && e.viability > 0.0 && !e.exploit_suspect);
            if ui.add_enabled(view.snapshot.live && viable && report.errors.is_empty(), egui::Button::new("Release into running world")).clicked() {
                let command = bestiary_daemon::Request::Release { genome: view.designer.genome.clone(), position: view.designer.position, count: view.designer.population };
                view.designer.status = if feed.tx.send(Job::Release(command)).is_ok() { "Releasing founding population…" } else { "World connection unavailable" }.into();
            }
            if !view.snapshot.live { ui.small("Start this world's daemon to release creatures."); }
            if !viable { ui.small("Release requires a current viable combine with a clear energy audit."); }
            ui.separator();
            ui.heading("Save, load and share");
            ui.text_edit_singleline(&mut view.designer.path);
            ui.horizontal(|ui| {
                if ui.button("Save genome").clicked() { let r = view.designer.save(&config); result(&mut view.designer, r); }
                if ui.button("Load genome").clicked() { let r = view.designer.load(&config); result(&mut view.designer, r); }
                if ui.button("Copy share string").clicked() { view.designer.copy_share(); ctx.copy_text(view.designer.share.clone()); }
            });
            ui.add(egui::TextEdit::multiline(&mut view.designer.share).desired_rows(2).hint_text("Paste a genome share string"));
            if ui.button("Load share string").clicked() { let r = view.designer.load_share(&config); result(&mut view.designer, r); }
            if let Some(error) = &view.designer.error { ui.colored_label(egui::Color32::LIGHT_RED, error); }
            if !view.designer.status.is_empty() { ui.label(&view.designer.status); }
        });
    });
    if before != view.designer.genome.hash() {
        view.dirty = true;
        view.replay_frame = 0;
        view.play_time = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attachment_points_keep_the_selected_face_at_edges_and_corners() {
        assert_eq!(selected_surface_face([1.0, 0.0, 0.0], None), 0);
        let selected_z = SurfaceControl {
            face: 4,
            point: [1.0, 0.0, 1.0],
        };
        assert_eq!(selected_surface_face(selected_z.point, Some(selected_z)), 4);
        let moved_x = SurfaceControl {
            face: 4,
            point: [0.2, 0.0, 1.0],
        };
        assert_eq!(selected_surface_face(moved_x.point, Some(moved_x)), 4);
        assert_eq!(selected_surface_face([0.0, -1.0, 0.0], Some(moved_x)), 3);
    }
}
