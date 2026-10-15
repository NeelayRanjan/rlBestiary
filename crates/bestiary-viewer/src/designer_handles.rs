//! Surface-constrained handles for the native 3D designer. These edit genes only;
//! all body poses come from normal deterministic expansion.
use super::*;
use bestiary_core::genome::{attachment_point, Body, Part, Route, Shape};
use bevy_egui::{egui, EguiContexts};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Endpoint {
    JointParent { bone: usize, edge: usize },
    JointChild { bone: usize, edge: usize },
    MuscleOrigin { bone: usize, muscle: usize },
    MuscleInsertion { bone: usize, muscle: usize },
}
#[derive(Clone)]
struct Handle {
    endpoint: Endpoint,
    part: Part,
    pose: Pose,
    point: [f32; 3],
    label: &'static str,
    color: egui::Color32,
    offset: egui::Vec2,
}
struct Drag {
    handle: Handle,
    /// Preserve the initial cursor-to-surface displacement, including label
    /// separation and off-center grabs, instead of jumping to a new surface.
    offset: egui::Vec2,
}
#[derive(Default)]
pub(super) struct Interaction {
    drag: Option<Drag>,
    origin: Option<(usize, [f32; 3], u64)>,
}

fn route_target(body: &Body, start: usize, route: &[Route]) -> Option<usize> {
    route.iter().try_fold(start, |part, step| match step {
        Route::Parent => body.parts.get(part)?.parent,
        Route::Child(slot) => body.parts.get(part)?.children.get(*slot).copied().flatten(),
    })
}

fn handles(view: &View, body: &Body, poses: &[Pose]) -> Vec<Handle> {
    let d = &view.designer;
    let Some((index, part)) = body
        .parts
        .iter()
        .enumerate()
        .find(|(_, p)| p.gene == d.bone)
    else {
        return vec![];
    };
    let Some(bone) = d.genome.bones.get(d.bone) else {
        return vec![];
    };
    let mut result = vec![];
    let mut push = |endpoint, index: usize, point, label, color, offset| {
        if let (Some(part), Some(pose)) = (body.parts.get(index), poses.get(index)) {
            result.push(Handle {
                endpoint,
                part: part.clone(),
                pose: pose.clone(),
                point,
                label,
                color,
                offset,
            });
        }
    };
    if let Some(edge) = bone.edges.get(d.edge) {
        push(
            Endpoint::JointParent {
                bone: d.bone,
                edge: d.edge,
            },
            index,
            edge.attachment,
            "Jp",
            egui::Color32::GOLD,
            egui::vec2(-9.0, -8.0),
        );
        if let Some(child) = part.children.get(d.edge).copied().flatten() {
            push(
                Endpoint::JointChild {
                    bone: d.bone,
                    edge: d.edge,
                },
                child,
                edge.child_attachment,
                "Jc",
                egui::Color32::LIGHT_BLUE,
                egui::vec2(9.0, 8.0),
            );
        }
    }
    if let Some(muscle) = bone.muscles.get(d.muscle) {
        push(
            Endpoint::MuscleOrigin {
                bone: d.bone,
                muscle: d.muscle,
            },
            index,
            muscle.origin,
            "Mo",
            egui::Color32::LIGHT_GREEN,
            egui::Vec2::ZERO,
        );
        if let Some(target) = route_target(body, index, &muscle.route) {
            push(
                Endpoint::MuscleInsertion {
                    bone: d.bone,
                    muscle: d.muscle,
                },
                target,
                muscle.insertion,
                "Mi",
                egui::Color32::LIGHT_RED,
                egui::Vec2::ZERO,
            );
        }
    }
    result
}

fn world_point(part: &Part, pose: &Pose, normalized: [f32; 3]) -> Vec3 {
    let point = attachment_point(part.size, normalized, part.reflected, part.shape);
    Vec3::from_array(pose.translation)
        + Quat::from_array(pose.rotation) * Vec3::new(point.x, point.y, point.z)
}

/// Return the nearest forward ray/shape surface intersection, including rays
/// starting inside the shape. Capsule geometry matches the collider dimensions.
fn local_surface_hit(origin: Vec3, direction: Vec3, size: [f32; 3], shape: Shape) -> Option<f32> {
    if !origin.is_finite()
        || !direction.is_finite()
        || direction.length_squared() < 1e-12
        || size.iter().any(|x| !x.is_finite() || *x <= 0.0)
    {
        return None;
    }
    if shape == Shape::Box {
        let half = Vec3::from_array(size) * 0.5;
        let mut near = f32::NEG_INFINITY;
        let mut far = f32::INFINITY;
        for axis in 0..3 {
            if direction[axis].abs() < 1e-8 {
                if origin[axis].abs() > half[axis] {
                    return None;
                }
            } else {
                let a = (-half[axis] - origin[axis]) / direction[axis];
                let b = (half[axis] - origin[axis]) / direction[axis];
                near = near.max(a.min(b));
                far = far.min(a.max(b));
            }
        }
        if near > far || far < 0.0 {
            None
        } else {
            Some(if near >= 0.0 { near } else { far })
        }
    } else {
        let radius = size[0].min(size[2]) * 0.5;
        let half_segment = (size[1] * 0.5 - radius).max(0.0);
        let mut hits = vec![];
        let a = direction.x * direction.x + direction.z * direction.z;
        let b = origin.x * direction.x + origin.z * direction.z;
        let c = origin.x * origin.x + origin.z * origin.z - radius * radius;
        if a > 1e-12 {
            let discriminant = b * b - a * c;
            if discriminant >= 0.0 {
                for t in [
                    (-b - discriminant.sqrt()) / a,
                    (-b + discriminant.sqrt()) / a,
                ] {
                    if t >= 0.0 && (origin.y + direction.y * t).abs() <= half_segment {
                        hits.push(t);
                    }
                }
            }
        }
        for sign in [-1.0, 1.0] {
            let offset = origin - Vec3::Y * half_segment * sign;
            let a = direction.length_squared();
            let b = offset.dot(direction);
            let c = offset.length_squared() - radius * radius;
            let discriminant = b * b - a * c;
            if discriminant >= 0.0 {
                for t in [
                    (-b - discriminant.sqrt()) / a,
                    (-b + discriminant.sqrt()) / a,
                ] {
                    if t >= 0.0 && (origin.y + direction.y * t) * sign >= half_segment {
                        hits.push(t);
                    }
                }
            }
        }
        hits.into_iter().min_by(f32::total_cmp)
    }
}

/// Inverse of attachment_point on the physical surface. Capsule endpoints are
/// encoded by extending out from the nearest point on its central segment to the
/// bounding box; simply normalizing the hit vector would move the attachment.
fn normalized_surface(local: Vec3, part: &Part) -> [f32; 3] {
    let half = Vec3::from_array(part.size) * 0.5;
    let mut point = local;
    if part.shape == Shape::Capsule {
        let radius = part.size[0].min(part.size[2]) * 0.5;
        let half_segment = (half.y - radius).max(0.0);
        let center = Vec3::Y * local.y.clamp(-half_segment, half_segment);
        let direction = local - center;
        let factor = (0..3)
            .filter(|axis| direction[*axis].abs() > 1e-8)
            .map(|axis| (half[axis] * direction[axis].signum() - center[axis]) / direction[axis])
            .min_by(f32::total_cmp)
            .unwrap_or(1.0);
        point = center + direction * factor;
    }
    point /= half;
    point = point.clamp(Vec3::splat(-1.0), Vec3::ONE);
    let axis = (0..3)
        .max_by(|a, b| point[*a].abs().total_cmp(&point[*b].abs()))
        .unwrap_or(0);
    point[axis] = if point[axis] < 0.0 { -1.0 } else { 1.0 };
    if part.reflected {
        point.z = -point.z;
    }
    point.to_array()
}

fn surface_hit(ray: Ray3d, part: &Part, pose: &Pose) -> Option<(f32, [f32; 3])> {
    let inverse = Quat::from_array(pose.rotation).inverse();
    let origin = inverse * (ray.origin - Vec3::from_array(pose.translation));
    let direction = inverse * *ray.direction;
    let distance = local_surface_hit(origin, direction, part.size, part.shape)?;
    Some((
        distance,
        normalized_surface(origin + direction * distance, part),
    ))
}

fn apply_endpoint(
    genome: &mut bestiary_core::genome::Genome,
    endpoint: Endpoint,
    point: [f32; 3],
) -> bool {
    let target = match endpoint {
        Endpoint::JointParent { bone, edge } => genome
            .bones
            .get_mut(bone)
            .and_then(|b| b.edges.get_mut(edge))
            .map(|e| &mut e.attachment),
        Endpoint::JointChild { bone, edge } => genome
            .bones
            .get_mut(bone)
            .and_then(|b| b.edges.get_mut(edge))
            .map(|e| &mut e.child_attachment),
        Endpoint::MuscleOrigin { bone, muscle } => genome
            .bones
            .get_mut(bone)
            .and_then(|b| b.muscles.get_mut(muscle))
            .map(|m| &mut m.origin),
        Endpoint::MuscleInsertion { bone, muscle } => genome
            .bones
            .get_mut(bone)
            .and_then(|b| b.muscles.get_mut(muscle))
            .map(|m| &mut m.insertion),
    };
    if let Some(target) = target {
        if *target != point {
            *target = point;
            return true;
        }
    }
    false
}

pub(super) fn draw(
    mut view: ResMut<View>,
    mut contexts: EguiContexts,
    camera: Query<(&Camera, &GlobalTransform), With<scene::SceneCamera>>,
    mut interaction: Local<Interaction>,
) {
    if view.tab != Tab::Designer || view.designer.preview || view.clip.is_some() {
        *interaction = Interaction::default();
        return;
    }
    let Ok(body) = view
        .designer
        .genome
        .expand(&view.snapshot.world.config.genome)
    else {
        return;
    };
    let Ok((camera, transform)) = camera.get_single() else {
        return;
    };
    let Some(viewport_size) = camera.logical_viewport_size() else {
        return;
    };
    let ctx = contexts.ctx_mut();
    let rect = ctx.available_rect();
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let poses = scene::body_rest_poses(&body);
    let project = |point| -> Option<egui::Pos2> {
        let p = camera.world_to_viewport(transform, point).ok()?;
        Some(
            rect.min
                + egui::vec2(
                    p.x / viewport_size.x * rect.width(),
                    p.y / viewport_size.y * rect.height(),
                ),
        )
    };
    let ray_at = |point: egui::Pos2| -> Option<Ray3d> {
        let p = point - rect.min;
        camera
            .viewport_to_world(
                transform,
                Vec2::new(
                    p.x / rect.width() * viewport_size.x,
                    p.y / rect.height() * viewport_size.y,
                ),
            )
            .ok()
    };
    let painter = ctx
        .layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("designer attachment lines"),
        ))
        .with_clip_rect(rect);
    for (index, handle) in handles(&view, &body, &poses).into_iter().enumerate() {
        let Some(surface) = project(world_point(&handle.part, &handle.pose, handle.point)) else {
            continue;
        };
        let center = surface + handle.offset;
        if !rect.contains(center) {
            continue;
        }
        painter.line_segment([surface, center], egui::Stroke::new(1.0, handle.color));
        egui::Area::new(egui::Id::new(("designer attachment", index)))
            .order(egui::Order::Foreground)
            .fixed_pos(center - egui::vec2(8.0, 8.0))
            .movable(false)
            .show(ctx, |ui| {
                let (r, response) =
                    ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click_and_drag());
                ui.painter().circle_filled(r.center(), 6.0, handle.color);
                ui.painter().text(
                    r.center() + egui::vec2(0.0, -13.0),
                    egui::Align2::CENTER_CENTER,
                    handle.label,
                    egui::FontId::proportional(10.0),
                    handle.color,
                );
                if response.drag_started() {
                    let pressed = ctx.input(|i| i.pointer.press_origin()).unwrap_or(center);
                    interaction.drag = Some(Drag {
                        handle: handle.clone(),
                        offset: pressed - surface,
                    });
                }
                response.on_hover_text("Drag along this bone's surface");
            });
    }
    let (pointer, down, clicked, shift, escape) = ctx.input(|i| {
        (
            i.pointer.interact_pos(),
            i.pointer.primary_down(),
            i.pointer.primary_clicked(),
            i.modifiers.shift,
            i.key_pressed(egui::Key::Escape),
        )
    });
    if escape {
        *interaction = Interaction::default();
    }
    if !down {
        interaction.drag = None;
    }
    if let (Some(drag), Some(pointer)) = (&interaction.drag, pointer) {
        let handle = &drag.handle;
        if let Some((_, point)) =
            ray_at(pointer - drag.offset).and_then(|r| surface_hit(r, &handle.part, &handle.pose))
        {
            if apply_endpoint(&mut view.designer.genome, handle.endpoint, point) {
                let config = view.snapshot.world.config.clone();
                view.designer.changed(&config);
                view.dirty = true;
            }
        }
    }
    if interaction
        .origin
        .is_some_and(|(_, _, generation)| generation != view.designer.generation)
    {
        interaction.origin = None;
    }
    if let Some((part, point, _)) = interaction.origin {
        if let (Some(a), Some(b)) = (
            body.parts
                .get(part)
                .zip(poses.get(part))
                .and_then(|(p, pose)| project(world_point(p, pose, point))),
            pointer,
        ) {
            painter.line_segment([a, b], egui::Stroke::new(2.0, egui::Color32::LIGHT_GREEN));
        }
    }
    if clicked && shift && interaction.drag.is_none() {
        if let Some(pointer) = pointer.filter(|p| rect.contains(*p)) {
            let hit = ray_at(pointer).and_then(|ray| {
                body.parts
                    .iter()
                    .zip(&poses)
                    .enumerate()
                    .filter_map(|(i, (part, pose))| {
                        surface_hit(ray, part, pose).map(|(t, point)| (t, i, point))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
            });
            if let Some((_, part, point)) = hit {
                if let Some((origin, a, _)) = interaction.origin.take() {
                    let config = view.snapshot.world.config.clone();
                    match view
                        .designer
                        .connect_muscle(origin, part, a, point, &config)
                    {
                        Ok(()) => {
                            view.dirty = true;
                            view.error.clear();
                        }
                        Err(error) => view.error = error,
                    }
                } else {
                    interaction.origin = Some((part, point, view.designer.generation));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn designer_drag_attachment_points_stays_on_box_and_capsule_surfaces() {
        let config = bestiary_sim::config::Config::bundled();
        let genome = bestiary_core::genome::Genome::starter(&config.genome);
        let mut part = genome.expand(&config.genome).unwrap().parts[0].clone();
        part.size = [0.6, 2.0, 0.8];
        for shape in [Shape::Box, Shape::Capsule] {
            part.shape = shape;
            for reflected in [false, true] {
                part.reflected = reflected;
                for original in [
                    [1.0, 0.4, 0.2],
                    [0.3, 1.0, 0.4],
                    [0.2, -1.0, -0.5],
                    [0.1, 0.6, -1.0],
                ] {
                    let p = attachment_point(part.size, original, reflected, shape);
                    let encoded = normalized_surface(Vec3::new(p.x, p.y, p.z), &part);
                    assert!(encoded.iter().any(|v| v.abs() == 1.0));
                    assert!(encoded.iter().all(|v| v.abs() <= 1.0));
                    let again = attachment_point(part.size, encoded, reflected, shape);
                    assert!(
                        (p - again).norm() < 1e-5,
                        "{shape:?}, {original:?} => {encoded:?}"
                    );
                }
            }
        }
    }
    #[test]
    fn designer_surface_picking_handles_capsule_caps_inside_rays_and_misses() {
        let size = [2.0, 4.0, 2.0];
        for shape in [Shape::Box, Shape::Capsule] {
            assert_eq!(
                local_surface_hit(Vec3::new(4.0, 0.0, 0.0), -Vec3::X, size, shape),
                Some(3.0)
            );
            assert_eq!(
                local_surface_hit(Vec3::new(0.0, 4.0, 0.0), -Vec3::Y, size, shape),
                Some(2.0)
            );
            assert_eq!(
                local_surface_hit(Vec3::ZERO, Vec3::X, size, shape),
                Some(1.0)
            );
            assert_eq!(
                local_surface_hit(Vec3::new(4.0, 0.0, 0.0), Vec3::Y, size, shape),
                None
            );
            assert_eq!(local_surface_hit(Vec3::ZERO, Vec3::ZERO, size, shape), None);
            assert_eq!(local_surface_hit(Vec3::NAN, Vec3::X, size, shape), None);
        }
        assert_eq!(
            local_surface_hit(Vec3::new(0.9, 1.9, 5.0), -Vec3::Z, size, Shape::Capsule),
            None
        );
        assert!(local_surface_hit(Vec3::new(0.9, 1.9, 5.0), -Vec3::Z, size, Shape::Box).is_some());
    }
    #[test]
    fn designer_surface_picking_respects_rotated_reflected_body_coordinates() {
        let config = bestiary_sim::config::Config::bundled();
        let genome = bestiary_core::genome::Genome::starter(&config.genome);
        let mut part = genome.expand(&config.genome).unwrap().parts[0].clone();
        part.size = [2.0, 4.0, 2.0];
        part.reflected = true;
        let rotation = Quat::from_rotation_y(0.73) * Quat::from_rotation_z(-0.2);
        let pose = Pose {
            translation: [5.0, 2.0, -3.0],
            rotation: rotation.to_array(),
        };
        for shape in [Shape::Box, Shape::Capsule] {
            part.shape = shape;
            let ray = Ray3d::new(
                Vec3::from_array(pose.translation) + rotation * Vec3::new(4.0, 0.3, 0.2),
                Dir3::new(rotation * -Vec3::X).unwrap(),
            );
            let (distance, normalized) = surface_hit(ray, &part, &pose).unwrap();
            let reconstructed = world_point(&part, &pose, normalized);
            assert!(reconstructed.distance(ray.get_point(distance)) < 1e-5);
            assert!(
                normalized[2] < 0.0,
                "reflection belongs to gene coordinates"
            );
        }
    }
    #[test]
    fn designer_drag_edits_each_joint_and_muscle_endpoint_without_touching_other_genes() {
        let config = bestiary_sim::config::Config::bundled();
        let original = bestiary_core::genome::Genome::starter(&config.genome);
        let muscle_bone = original
            .bones
            .iter()
            .position(|bone| !bone.muscles.is_empty())
            .unwrap();
        let endpoints = [
            Endpoint::JointParent { bone: 0, edge: 0 },
            Endpoint::JointChild { bone: 0, edge: 0 },
            Endpoint::MuscleOrigin {
                bone: muscle_bone,
                muscle: 0,
            },
            Endpoint::MuscleInsertion {
                bone: muscle_bone,
                muscle: 0,
            },
        ];
        for endpoint in endpoints {
            let mut genome = original.clone();
            assert!(apply_endpoint(&mut genome, endpoint, [1.0, 0.32, -0.78]));
            assert!(!apply_endpoint(&mut genome, endpoint, [1.0, 0.32, -0.78]));
            assert_eq!(genome.life, original.life);
            let edited_bone = match endpoint {
                Endpoint::JointParent { bone, .. }
                | Endpoint::JointChild { bone, .. }
                | Endpoint::MuscleOrigin { bone, .. }
                | Endpoint::MuscleInsertion { bone, .. } => bone,
            };
            for (index, bone) in genome.bones.iter().enumerate() {
                if index != edited_bone {
                    assert_eq!(bone, &original.bones[index]);
                }
            }
            assert!(genome
                .expand(&config.genome)
                .unwrap()
                .actuated_axes_covered());
        }
        let mut genome = original.clone();
        assert!(!apply_endpoint(
            &mut genome,
            Endpoint::JointParent {
                bone: usize::MAX,
                edge: 0
            },
            [1.0, 0.0, 0.0]
        ));
        assert_eq!(genome, original);
    }
}
