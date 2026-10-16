use super::*;
use bestiary_core::genome::{Body, Shape};
use bevy::{
    core_pipeline::tonemapping::Tonemapping,
    input::mouse::{MouseMotion, MouseWheel},
    render::camera::Viewport,
};
use bevy_egui::EguiContexts;
#[derive(Component)]
pub struct SceneCamera;
#[derive(Component)]
pub(crate) struct Visual;
#[derive(Component)]
pub(crate) struct Proxy;
#[derive(Component)]
pub(crate) struct ReplayTarget {
    body_parts: usize,
}
#[derive(Component)]
pub(crate) struct BodyPart {
    hash: [u8; 32],
    pub(crate) index: usize,
    offset: Vec3,
    base: Pose,
    source: PoseSource,
    local_rotation: Quat,
    local_translation: Vec3,
}
#[derive(Component)]
pub(crate) struct MuscleVisual {
    hash: [u8; 32],
    index: usize,
    offset: Vec3,
    source: PoseSource,
    origin_part: usize,
    insertion_part: usize,
    origin: Vec3,
    insertion: Vec3,
    bases: [Pose; 2],
    rest: f32,
    area: f32,
}
/// Where a drawn body takes its poses from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PoseSource {
    /// The inspector or designer body: live physics or the selected recording.
    Inspector,
    /// A world-view body near the camera, pinned in place, playing its sprint.
    WorldSprint,
    /// A training lane, by index into `TrainingView::slots`.
    Training(usize),
}
#[derive(Resource)]
pub struct Palette {
    cube: Handle<Mesh>,
    bone: Handle<Mesh>,
    torso: Handle<Mesh>,
    muscle: Handle<Mesh>,
    capsule: Handle<Mesh>,
    proxy: Handle<Mesh>,
    colors: BTreeMap<[u8; 3], Handle<StandardMaterial>>,
    body_meshes: BTreeMap<[u32; 3], Handle<Mesh>>,
    muscle_materials: BTreeMap<u8, Handle<StandardMaterial>>,
}
impl Palette {
    fn material(
        &mut self,
        rgb: [f32; 3],
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        let key = rgb.map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8);
        self.colors
            .entry(key)
            .or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: Color::srgb(rgb[0], rgb[1], rgb[2]),
                    perceptual_roughness: 0.9,
                    ..default()
                })
            })
            .clone()
    }
}
pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut contexts: EguiContexts,
    appearance: Res<anatomy::Appearance>,
) {
    contexts
        .ctx_mut()
        .set_visuals(bevy_egui::egui::Visuals::dark());
    let mut style = (*contexts.ctx_mut().style()).clone();
    style.visuals.panel_fill = bevy_egui::egui::Color32::from_rgb(15, 23, 29);
    style.visuals.window_fill = bevy_egui::egui::Color32::from_rgb(18, 28, 35);
    style.visuals.selection.bg_fill = bevy_egui::egui::Color32::from_rgb(29, 88, 82);
    style.spacing.item_spacing = bevy_egui::egui::vec2(10.0, 8.0);
    contexts.ctx_mut().set_style(style);
    commands.insert_resource(Palette {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        bone: meshes.add(anatomy::tissue_mesh(&appearance, false)),
        torso: meshes.add(anatomy::torso_mesh(&appearance)),
        muscle: meshes.add(anatomy::tissue_mesh(&appearance, true)),
        capsule: meshes.add(Capsule3d::new(0.5, 0.0)),
        proxy: meshes.add(anatomy::proxy_mesh(&appearance)),
        colors: BTreeMap::new(),
        body_meshes: BTreeMap::new(),
        muscle_materials: BTreeMap::new(),
    });
    commands.spawn((
        Camera3d::default(),
        Transform::default(),
        Tonemapping::ReinhardLuminance,
        SceneCamera,
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 12000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(50.0, 100.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
pub fn species_color(id: u64) -> [f32; 3] {
    const C: [[f32; 3]; 8] = [
        [0.42, 0.83, 0.68],
        [0.92, 0.64, 0.37],
        [0.43, 0.66, 0.89],
        [0.77, 0.53, 0.83],
        [0.85, 0.85, 0.48],
        [0.86, 0.47, 0.47],
        [0.40, 0.81, 0.81],
        [0.72, 0.73, 0.80],
    ];
    C[id as usize % C.len()]
}
fn terrain_color(
    c: &bestiary_world::Cell,
    overlay: Overlay,
    water: bool,
    plant_nutrients: i64,
) -> [f32; 3] {
    if water && matches!(overlay, Overlay::Species | Overlay::Predation) {
        return [0.10, 0.27, 0.37];
    }
    let (v, low, high) = match overlay {
        Overlay::Temperature => (c.temperature, [0.18, 0.34, 0.6], [0.85, 0.42, 0.20]),
        Overlay::Moisture => (c.moisture, [0.43, 0.30, 0.20], [0.22, 0.55, 0.53]),
        Overlay::Pheromone(i) => (
            c.pheromones[i] / (1.0 + c.pheromones[i]),
            [0.10, 0.13, 0.16],
            [0.72, 0.43, 0.89],
        ),
        _ => (
            c.plant.nutrients as f32 / (c.plant.nutrients as f32 + plant_nutrients.max(1) as f32),
            [0.20, 0.25, 0.18],
            [0.31, 0.51, 0.30],
        ),
    };
    std::array::from_fn(|i| {
        low[i] + (high[i] - low[i]) * ((v * 16.0).round() / 16.0).clamp(0.0, 1.0)
    })
}
/// Highlight actual participants in the latest configured decision interval.
fn recent_predation(
    events: &[(u64, bestiary_world::EcologyEvent)],
    tick: u64,
    interval: u64,
) -> BTreeSet<u64> {
    let first = tick.saturating_sub(interval.saturating_sub(1));
    events
        .iter()
        .filter(|(at, _)| *at >= first && *at <= tick)
        .flat_map(|(_, event)| match event {
            bestiary_world::EcologyEvent::Predation { attacker, victim } => {
                vec![*attacker, *victim]
            }
            _ => vec![],
        })
        .collect()
}
/// The strike trial records its dynamic target after every articulated body part.
pub(super) fn strike_target_pose(
    replay: &Replay,
    frame: usize,
    body_parts: usize,
) -> Option<&Pose> {
    (replay.kind == TrialKind::Strike && body_parts > 0)
        .then(|| replay.frames.get(frame)?.get(body_parts))
        .flatten()
}
#[allow(clippy::too_many_arguments)]
fn draw_body(
    commands: &mut Commands,
    palette: &mut Palette,
    materials: &mut Assets<StandardMaterial>,
    meshes: &mut Assets<Mesh>,
    hash: [u8; 32],
    body: &Body,
    genome: &bestiary_core::genome::Genome,
    offset: Vec3,
    source: PoseSource,
    color_override: Option<[f32; 3]>,
    anatomy: bool,
    appearance: &anatomy::Appearance,
) {
    let spawn_lift = bestiary_sim::genome::spawn_lift(body);
    for (index, part) in body.parts.iter().enumerate() {
        let mut translation: [f32; 3] = part.pose.translation.vector.into();
        translation[1] += spawn_lift;
        let pose = Pose {
            translation,
            rotation: part.pose.rotation.coords.into(),
        };
        let color = color_override.unwrap_or(genome.bones[part.gene].color);
        let material = palette.material(
            if anatomy {
                appearance.bone_tint(color)
            } else {
                color
            },
            materials,
        );
        let (mesh, scale) = match part.shape {
            Shape::Box => (palette.cube.clone(), Vec3::from_array(part.size)),
            Shape::Capsule => {
                let key = part.size.map(f32::to_bits);
                let handle = palette
                    .body_meshes
                    .entry(key)
                    .or_insert_with(|| {
                        let radius = part.size[0].min(part.size[2]) * 0.5;
                        meshes.add(Capsule3d::new(
                            radius,
                            (part.size[1] - 2.0 * radius).max(0.0),
                        ))
                    })
                    .clone();
                (handle, Vec3::ONE)
            }
        };
        let (local_rotation, anatomy_scale) = anatomy::bone_frame(part.size);
        // A terminal segment can bear weight on any collider face. Retain that
        // envelope so the visible foot meets the floor at physical contact,
        // including when a box is tilted onto a corner or an edge.
        let terminal = !body.parts.iter().any(|child| child.parent == Some(index));
        let skeletal_limb = anatomy && part.parent.is_some() && !terminal;
        let local_rotation = if skeletal_limb {
            local_rotation
        } else {
            Quat::IDENTITY
        };
        let (mesh, scale) = if skeletal_limb {
            (palette.bone.clone(), anatomy_scale)
        } else if anatomy && part.parent.is_none() && part.shape == Shape::Box {
            (palette.torso.clone(), Vec3::from_array(part.size))
        } else {
            (mesh, scale)
        };
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform {
                translation: Vec3::from_array(pose.translation) + offset,
                rotation: Quat::from_array(pose.rotation) * local_rotation,
                scale,
            },
            Visual,
            BodyPart {
                hash,
                index,
                offset,
                base: pose.clone(),
                source,
                local_rotation,
                local_translation: Vec3::ZERO,
            },
        ));
        if anatomy {
            if let Some(edge) = &part.incoming {
                let local = bestiary_core::genome::attachment_point(
                    part.size,
                    edge.child_attachment,
                    part.reflected,
                    part.shape,
                );
                let local_translation = Vec3::new(local.x, local.y, local.z);
                let radius = part.size.into_iter().fold(f32::INFINITY, f32::min)
                    * appearance.joint_radius_fraction;
                let material = palette.material(appearance.bone_tint(color), materials);
                commands.spawn((
                    Mesh3d(palette.capsule.clone()),
                    MeshMaterial3d(material),
                    Transform::from_translation(
                        Vec3::from_array(pose.translation)
                            + Quat::from_array(pose.rotation) * local_translation
                            + offset,
                    )
                    .with_scale(Vec3::splat(radius * 2.0)),
                    Visual,
                    BodyPart {
                        hash,
                        index,
                        offset,
                        base: pose,
                        source,
                        local_rotation: Quat::IDENTITY,
                        local_translation,
                    },
                ));
            }
        }
    }
    if anatomy {
        let material = palette.material([1.0; 3], materials);
        for (index, m) in body.muscles.iter().enumerate() {
            let bases = [m.origin_part, m.insertion_part].map(|i| {
                let mut translation: [f32; 3] = body.parts[i].pose.translation.vector.into();
                translation[1] += spawn_lift;
                Pose {
                    translation,
                    rotation: body.parts[i].pose.rotation.coords.into(),
                }
            });
            let origin = Vec3::from_array(m.origin);
            let insertion = Vec3::from_array(m.insertion);
            let a = pose_point(&bases[0], origin);
            let b = pose_point(&bases[1], insertion);
            let rest = a.distance(b);
            let transform =
                anatomy::muscle_transform(a + offset, b + offset, rest, m.area, appearance)
                    .unwrap_or_default();
            commands.spawn((
                Mesh3d(palette.muscle.clone()),
                MeshMaterial3d(material.clone()),
                transform,
                Visual,
                MuscleVisual {
                    hash,
                    index,
                    offset,
                    source,
                    origin_part: m.origin_part,
                    insertion_part: m.insertion_part,
                    origin,
                    insertion,
                    bases,
                    rest,
                    area: m.area,
                },
            ));
        }
    }
}
#[allow(clippy::too_many_arguments)]
pub fn rebuild(
    mut commands: Commands,
    mut view: ResMut<View>,
    feed: Res<Feed>,
    mut palette: ResMut<Palette>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    old: Query<Entity, With<Visual>>,
    mut previous_near: Local<BTreeSet<u64>>,
    appearance: Res<anatomy::Appearance>,
) {
    let near_ids = nearest_bodies(&view.snapshot.world, view.target, 20);
    if view.tab == Tab::World && !view.benchmark && *previous_near != near_ids {
        view.dirty = true;
    }
    if !view.dirty {
        return;
    }
    view.dirty = false;
    *previous_near = near_ids.clone();
    for entity in &old {
        commands.entity(entity).despawn_recursive();
    }
    let snapshot = view.snapshot.clone();
    let world = &snapshot.world;
    if view.tab == Tab::Training {
        let floor = palette.material(appearance.ground_color, &mut materials);
        commands.spawn((
            Mesh3d(palette.cube.clone()),
            MeshMaterial3d(floor),
            Transform::from_xyz(0.0, -0.1, 0.0).with_scale(Vec3::new(200.0, 0.2, 200.0)),
            Visual,
        ));
        let slots = view.training.slots();
        let count = slots.len();
        for (slot, (genome, _, champion)) in slots.into_iter().enumerate() {
            if let Ok(body) = genome.expand(&world.config.genome) {
                draw_body(
                    &mut commands,
                    &mut palette,
                    &mut materials,
                    &mut meshes,
                    genome.hash(),
                    &body,
                    genome,
                    crate::ui_training::lane_offset(slot, count),
                    PoseSource::Training(slot),
                    crate::ui_training::ghost_color(champion),
                    view.anatomy && champion,
                    &appearance,
                );
            }
        }
        return;
    }
    if view.tab.is_body() {
        let floor = palette.material(appearance.ground_color, &mut materials);
        commands.spawn((
            Mesh3d(palette.cube.clone()),
            MeshMaterial3d(floor),
            Transform::from_xyz(0.0, -0.1, 0.0).with_scale(Vec3::new(200.0, 0.2, 200.0)),
            Visual,
        ));
        if let Some(genome) = view.body_genome() {
            if let Ok(body) = genome.expand(&world.config.genome) {
                draw_body(
                    &mut commands,
                    &mut palette,
                    &mut materials,
                    &mut meshes,
                    view.body_hash(),
                    &body,
                    genome,
                    Vec3::ZERO,
                    PoseSource::Inspector,
                    None,
                    view.anatomy,
                    &appearance,
                );
                let material = palette.material([0.9, 0.62, 0.22], &mut materials);
                commands.spawn((
                    Mesh3d(palette.cube.clone()),
                    MeshMaterial3d(material),
                    Transform::from_scale(Vec3::from_array(
                        bestiary_sim::combine::STRIKE_TARGET_SIZE,
                    )),
                    Visibility::Hidden,
                    Visual,
                    ReplayTarget {
                        body_parts: body.parts.len(),
                    },
                ));
            }
        }
        return;
    }
    if view.benchmark {
        let floor = palette.material([0.12, 0.17, 0.19], &mut materials);
        commands.spawn((
            Mesh3d(palette.cube.clone()),
            MeshMaterial3d(floor),
            Transform::from_xyz(30.0, -0.05, 30.0).with_scale(Vec3::new(65.0, 0.1, 65.0)),
            Visual,
        ));
        for i in 0..20 {
            let hash = world
                .agents
                .get(i % world.agents.len().max(1))
                .map(|a| a.genome)
                .unwrap_or(view.genome);
            if let Some(g) = world.genomes.get(&hash) {
                if let Ok(body) = g.genome.expand(&world.config.genome) {
                    draw_body(
                        &mut commands,
                        &mut palette,
                        &mut materials,
                        &mut meshes,
                        hash,
                        &body,
                        &g.genome,
                        Vec3::new(
                            20.0 + (i % 5) as f32 * 4.0,
                            0.0,
                            38.0 + (i / 5) as f32 * 4.0,
                        ),
                        PoseSource::WorldSprint,
                        None,
                        view.anatomy,
                        &appearance,
                    );
                    request_replay(&mut view, (hash, TrialKind::Sprint), &feed);
                }
            }
        }
        for i in 0..2000 {
            let material = palette.material(species_color((i % 8) as u64), &mut materials);
            commands.spawn((
                Mesh3d(palette.proxy.clone()),
                MeshMaterial3d(material),
                Transform::from_xyz((i % 50) as f32 * 1.2, 0.35, (i / 50) as f32 * 0.8)
                    .with_scale(Vec3::splat(0.7)),
                Visual,
                Proxy,
            ));
        }
        return;
    }
    let cell_size = world.config.world.cell_metres;
    for (i, c) in world.cells.iter().enumerate() {
        let water = c.height < world.sea_level;
        let top = c.height.max(world.sea_level) * 8.0;
        let color = terrain_color(c, view.overlay, water, world.config.world.plant_nutrients);
        let material = palette.material(color, &mut materials);
        commands.spawn((
            Mesh3d(palette.cube.clone()),
            MeshMaterial3d(material),
            Transform::from_xyz(
                (i % world.config.world.grid) as f32 * cell_size + cell_size * 0.5,
                top - 1.0,
                (i / world.config.world.grid) as f32 * cell_size + cell_size * 0.5,
            )
            .with_scale(Vec3::new(cell_size * 0.995, 2.0, cell_size * 0.995)),
            Visual,
        ));
    }
    let predation = recent_predation(
        &snapshot.events,
        world.tick,
        world.config.world.decision_interval,
    );
    for a in &world.agents {
        let g = &world.genomes[&a.genome];
        let height = world.cells[world.cell_at(a.position)]
            .height
            .max(world.sea_level)
            * 8.0;
        let offset = Vec3::new(a.position[0], height, a.position[1]);
        if near_ids.contains(&a.id) {
            if let Ok(body) = g.genome.expand(&world.config.genome) {
                draw_body(
                    &mut commands,
                    &mut palette,
                    &mut materials,
                    &mut meshes,
                    a.genome,
                    &body,
                    &g.genome,
                    offset,
                    PoseSource::WorldSprint,
                    match view.overlay {
                        Overlay::Species => Some(species_color(a.species)),
                        Overlay::Predation if predation.contains(&a.id) => Some([0.95, 0.25, 0.18]),
                        _ => None,
                    },
                    view.anatomy,
                    &appearance,
                );
                request_replay(&mut view, (a.genome, TrialKind::Sprint), &feed);
            }
        } else {
            let mut color = species_color(a.species);
            if view.overlay == Overlay::Predation && predation.contains(&a.id) {
                color = [0.95, 0.25, 0.18];
            }
            let material = palette.material(color, &mut materials);
            let size = (g.traits.mass * a.growth).cbrt().max(0.25) * 2.0;
            commands.spawn((
                Mesh3d(palette.proxy.clone()),
                MeshMaterial3d(material),
                Transform::from_translation(offset + Vec3::Y * size * 0.5).with_scale(Vec3::new(
                    size,
                    size * 0.7,
                    size,
                )),
                Visual,
                Proxy,
            ));
        }
    }
}

pub fn animate(
    view: Res<View>,
    inspector: NonSend<Inspector>,
    mut parts: Query<(&BodyPart, &mut Transform), Without<ReplayTarget>>,
    mut targets: Query<(&ReplayTarget, &mut Transform, &mut Visibility), Without<BodyPart>>,
) {
    for (part, mut transform) in &mut parts {
        let frames = source_frames(&view, &inspector, part.source, part.hash);
        let pose = frames.and_then(|f| f.get(part.index)).unwrap_or(&part.base);
        let root = frames
            .and_then(|f| f.first())
            .map(|p| p.translation)
            .unwrap_or([0.0; 3]);
        transform.translation = displayed_point(
            pose,
            part.local_translation,
            root,
            part.offset,
            part.source != PoseSource::WorldSprint,
        );
        transform.rotation = Quat::from_array(pose.rotation) * part.local_rotation;
    }
    for (target, mut transform, mut visible) in &mut targets {
        let replay = (!view.live_preview && body_preview_enabled(&view))
            .then(|| view.replays.get(&(view.body_hash(), view.trial)))
            .flatten();
        if let Some(pose) = replay
            .and_then(|replay| strike_target_pose(replay, view.replay_frame, target.body_parts))
        {
            *visible = Visibility::Visible;
            transform.translation = Vec3::from_array(pose.translation);
            transform.rotation = Quat::from_array(pose.rotation);
        } else {
            *visible = Visibility::Hidden;
        }
    }
}
fn pose_point(pose: &Pose, local: Vec3) -> Vec3 {
    Vec3::from_array(pose.translation) + Quat::from_array(pose.rotation) * local
}
/// Inspector poses stay in arena space: moving the camera must never move a
/// planted foot, muscle insertion or strike target relative to the ground.
fn displayed_point(
    pose: &Pose,
    local: Vec3,
    root: [f32; 3],
    offset: Vec3,
    inspector: bool,
) -> Vec3 {
    pose_point(pose, local) + offset
        - if inspector {
            Vec3::ZERO
        } else {
            Vec3::new(root[0], 0.0, root[2])
        }
}
pub(super) fn body_rest_poses(body: &Body) -> Vec<Pose> {
    let lift = bestiary_sim::genome::spawn_lift(body);
    body.parts
        .iter()
        .map(|part| {
            let mut translation: [f32; 3] = part.pose.translation.vector.into();
            translation[1] += lift;
            Pose {
                translation,
                rotation: part.pose.rotation.coords.into(),
            }
        })
        .collect()
}
fn source_frames<'a>(
    view: &'a View,
    inspector: &'a Inspector,
    source: PoseSource,
    hash: [u8; 32],
) -> Option<&'a [Pose]> {
    match source {
        PoseSource::Inspector => inspector_poses(view, inspector),
        PoseSource::WorldSprint => view
            .replays
            .get(&(hash, TrialKind::Sprint))
            .and_then(|r| {
                r.frames.get(playback::frame_index(
                    view.world_play_time,
                    r.dt,
                    r.frames.len(),
                ))
            })
            .map(Vec::as_slice),
        PoseSource::Training(slot) => view
            .training
            .slots()
            .get(slot)
            .and_then(|(_, r, _)| {
                r.frames.get(playback::clamped_frame_index(
                    view.training.time,
                    r.dt,
                    r.frames.len(),
                ))
            })
            .map(Vec::as_slice),
    }
}
fn body_preview_enabled(view: &View) -> bool {
    view.tab != Tab::Designer || view.designer.preview
}
fn inspector_poses<'a>(view: &'a View, inspector: &'a Inspector) -> Option<&'a [Pose]> {
    if !body_preview_enabled(view) {
        return None;
    }
    if view.live_preview {
        Some(inspector.poses.as_slice())
    } else {
        view.replays
            .get(&(view.body_hash(), view.trial))
            .and_then(|r| r.frames.get(view.replay_frame))
            .map(Vec::as_slice)
    }
}
/// A floor grid is anchored to arena coordinates; it exposes real translation
/// and slipping while the camera tracks the torso. It never alters a pose.
pub fn contact_ground(
    view: Res<View>,
    appearance: Res<anatomy::Appearance>,
    mut gizmos: Gizmos,
    mut lights: Query<&mut DirectionalLight>,
) {
    for mut light in &mut lights {
        light.shadows_enabled = view.tab.is_body() && appearance.inspector_shadows;
    }
    if !view.tab.is_body() {
        return;
    }
    let spacing = appearance.contact_grid_spacing;
    let radius = appearance.contact_grid_radius;
    let color = Color::srgb_from_array(appearance.grid_color);
    let center = (Vec2::new(view.target.x, view.target.z) / spacing).floor() * spacing;
    let steps = (radius / spacing).ceil() as i32;
    // Keep the grid just above the physical surface to avoid depth fighting.
    for i in -steps..=steps {
        let d = i as f32 * spacing;
        gizmos.line(
            Vec3::new(center.x + d, 0.001, center.y - radius),
            Vec3::new(center.x + d, 0.001, center.y + radius),
            color,
        );
        gizmos.line(
            Vec3::new(center.x - radius, 0.001, center.y + d),
            Vec3::new(center.x + radius, 0.001, center.y + d),
            color,
        );
    }
}

pub fn animate_tissue(
    view: Res<View>,
    inspector: NonSend<Inspector>,
    appearance: Res<anatomy::Appearance>,
    mut muscles: Query<(
        &MuscleVisual,
        &mut Transform,
        &mut Visibility,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
    mut palette: ResMut<Palette>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (muscle, mut transform, mut visibility, mut material) in &mut muscles {
        let frames = source_frames(&view, &inspector, muscle.source, muscle.hash);
        let root = frames
            .and_then(|f| f.first())
            .map(|p| Vec3::new(p.translation[0], 0.0, p.translation[2]))
            .unwrap_or(Vec3::ZERO);
        let point = |part, base: &Pose, local| {
            displayed_point(
                frames.and_then(|f| f.get(part)).unwrap_or(base),
                local,
                root.to_array(),
                muscle.offset,
                muscle.source != PoseSource::WorldSprint,
            )
        };
        let a = point(muscle.origin_part, &muscle.bases[0], muscle.origin);
        let b = point(muscle.insertion_part, &muscle.bases[1], muscle.insertion);
        if let Some(next) = anatomy::muscle_transform(a, b, muscle.rest, muscle.area, &appearance) {
            *transform = next;
            *visibility = Visibility::Inherited;
        } else {
            *visibility = Visibility::Hidden;
        }
        // Replay files contain poses, not activation: never fabricate a brain signal.
        let activation = if muscle.source == PoseSource::Inspector
            && view.live_preview
            && body_preview_enabled(&view)
        {
            inspector
                .creature
                .as_ref()
                .and_then(|c| {
                    let m = c.body.muscles.get(muscle.index)?;
                    c.brains.get(m.origin_part)?.values.get(m.output).copied()
                })
                .unwrap_or(0.0)
        } else {
            0.0
        };
        let activation = (activation.clamp(0.0, 1.0) * 15.0).round() / 15.0;
        material.0 = palette
            .muscle_materials
            .entry((activation * 15.0) as u8)
            .or_insert_with(|| materials.add(appearance.muscle_material(activation)))
            .clone();
    }
}

#[allow(clippy::too_many_arguments)]
pub fn camera(
    mut view: ResMut<View>,
    inspector: NonSend<Inspector>,
    mut contexts: EguiContexts,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut motion: EventReader<MouseMotion>,
    mut wheel: EventReader<MouseWheel>,
    mut camera: Query<(&mut Transform, &mut Camera), With<SceneCamera>>,
    time: Res<Time>,
) {
    let mouse = motion.read().fold(Vec2::ZERO, |sum, m| sum + m.delta);
    let scroll = wheel.read().map(|w| w.y).sum::<f32>();
    if view.clip.is_none() && !contexts.ctx_mut().wants_pointer_input() {
        if buttons.pressed(MouseButton::Right) {
            view.yaw -= mouse.x * 0.005;
            view.pitch = (view.pitch + mouse.y * 0.005).clamp(0.1, 1.5);
        }
        if scroll != 0.0 {
            view.radius = (view.radius * (-scroll * 0.1).exp()).clamp(2.0, 5000.0);
        }
    }
    if view.tab == Tab::Training {
        let count = view.training.slots().len();
        view.target = source_frames(&view, &inspector, PoseSource::Training(0), [0; 32])
            .and_then(|p| p.first())
            .map(|p| Vec3::from_array(p.translation) + crate::ui_training::lane_offset(0, count))
            .unwrap_or(Vec3::new(0.0, 0.7, 0.0));
    } else if view.tab.is_body() {
        view.target = inspector_poses(&view, &inspector)
            .and_then(|p| p.first())
            .map(|p| Vec3::from_array(p.translation))
            .unwrap_or(Vec3::new(0.0, 0.7, 0.0));
    } else if view.mode == CameraMode::Follow {
        if let Some(a) = view
            .snapshot
            .world
            .agents
            .iter()
            .find(|a| Some(a.id) == view.selected)
        {
            view.target = Vec3::new(a.position[0], 0.0, a.position[1]);
        }
    } else if view.clip.is_none()
        && view.mode == CameraMode::Free
        && !contexts.ctx_mut().wants_keyboard_input()
    {
        let mut movement = Vec3::ZERO;
        if keys.pressed(KeyCode::KeyW) {
            movement.z -= 1.0;
        }
        if keys.pressed(KeyCode::KeyS) {
            movement.z += 1.0;
        }
        if keys.pressed(KeyCode::KeyA) {
            movement.x -= 1.0;
        }
        if keys.pressed(KeyCode::KeyD) {
            movement.x += 1.0;
        }
        let speed = view.radius * 0.5 * time.delta_secs();
        view.target += movement * speed;
    }
    let rect = contexts.ctx_mut().available_rect();
    view.canvas_rect = [rect.min.x, rect.min.y, rect.width(), rect.height()];
    view.canvas_scale = contexts.ctx_mut().pixels_per_point();
    for (mut transform, _) in &mut camera {
        let direction = Vec3::new(
            view.yaw.sin() * view.pitch.cos(),
            view.pitch.sin(),
            view.yaw.cos() * view.pitch.cos(),
        );
        *transform = Transform::from_translation(view.target + direction * view.radius)
            .looking_at(view.target, Vec3::Y);
    }
}

/// Apply after Bevy processes scale-factor changes, so it cannot resize an already
/// physical viewport a second time on Retina/window initialization.
pub fn viewport(
    view: Res<View>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut cameras: Query<&mut Camera, With<SceneCamera>>,
    mut original_resizable: Local<Option<bool>>,
) {
    if let Ok(mut window) = windows.get_single_mut() {
        clip_window_resizing(
            view.clip.is_some(),
            &mut window.resizable,
            &mut original_resizable,
        );
    }
    let maximum = if view.benchmark {
        UVec2::new(2880, 1920)
    } else if let Ok(window) = windows.get_single() {
        UVec2::new(window.physical_width(), window.physical_height())
    } else {
        UVec2::new(2880, 1920)
    };
    if view.tab != Tab::World && !view.tab.is_body() {
        return;
    }
    let (position, size) = viewport_bounds(view.canvas_rect, view.canvas_scale, maximum);
    for mut camera in &mut cameras {
        camera.viewport = Some(Viewport {
            physical_position: position,
            physical_size: size,
            ..default()
        });
    }
}

fn clip_window_resizing(exporting: bool, resizable: &mut bool, original: &mut Option<bool>) {
    if exporting {
        original.get_or_insert(*resizable);
        *resizable = false;
    } else if let Some(saved) = original.take() {
        *resizable = saved;
    }
}

pub fn joints_and_muscles(
    view: Res<View>,
    inspector: NonSend<Inspector>,
    appearance: Res<anatomy::Appearance>,
    mut gizmos: Gizmos,
) {
    if !view.tab.is_body() || view.anatomy {
        return;
    }
    let Some(genome) = view.body_genome() else {
        return;
    };
    let Ok(body) = genome.expand(&view.snapshot.world.config.genome) else {
        return;
    };
    let rest_poses;
    let poses = if let Some(poses) = inspector_poses(&view, &inspector) {
        poses
    } else {
        rest_poses = body_rest_poses(&body);
        &rest_poses
    };
    if poses.is_empty() {
        return;
    }
    let point = |part: usize, local: [f32; 3]| -> Option<Vec3> {
        let pose = poses.get(part)?;
        Some(
            Vec3::from_array(pose.translation)
                + Quat::from_array(pose.rotation) * Vec3::from_array(local),
        )
    };
    for muscle in &body.muscles {
        let Some(a) = point(muscle.origin_part, muscle.origin) else {
            continue;
        };
        let Some(b) = point(muscle.insertion_part, muscle.insertion) else {
            continue;
        };
        let activation = if view.live_preview && body_preview_enabled(&view) {
            inspector
                .creature
                .as_ref()
                .and_then(|creature| creature.brains.get(muscle.origin_part))
                .and_then(|brain| brain.values.get(muscle.output))
                .copied()
                .unwrap_or(0.0)
                .clamp(0.0, 1.0)
        } else {
            0.0
        };
        gizmos.line(a, b, appearance.muscle_tint(activation));
    }
    for (i, p) in body.parts.iter().enumerate().skip(1) {
        if let Some(edge) = &p.incoming {
            let local = bestiary_core::genome::attachment_point(
                p.size,
                edge.child_attachment,
                p.reflected,
                p.shape,
            );
            if let Some(point) = point(i, [local.x, local.y, local.z]) {
                gizmos.sphere(
                    Isometry3d::from_translation(point),
                    0.025,
                    Color::srgb(0.9, 0.75, 0.3),
                );
            }
        }
    }
}

/// Select a stable set around the camera's focus; moving an offline camera must
/// update detail just as receiving a new daemon snapshot does.
pub(crate) fn nearest_bodies(
    world: &bestiary_world::World,
    target: Vec3,
    limit: usize,
) -> BTreeSet<u64> {
    let mut near: Vec<_> = world.agents.iter().collect();
    near.sort_by(|a, b| {
        let distance = |a: &bestiary_world::Agent| {
            Vec3::new(a.position[0], 0.0, a.position[1]).distance_squared(target)
        };
        distance(a).total_cmp(&distance(b)).then(a.id.cmp(&b.id))
    });
    near.into_iter().take(limit).map(|a| a.id).collect()
}
fn viewport_bounds(rect: [f32; 4], scale: f32, maximum: UVec2) -> (UVec2, UVec2) {
    let [x, y, width, height] = rect;
    let maximum = maximum.max(UVec2::ONE);
    let position = UVec2::new(
        (x * scale).round().max(0.0) as u32,
        (y * scale).round().max(0.0) as u32,
    )
    .min(maximum - UVec2::ONE);
    let size = UVec2::new(
        (width * scale).floor().max(1.0) as u32,
        (height * scale).floor().max(1.0) as u32,
    )
    .min(maximum - position);
    (position, size)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn designer_rest_pose_uses_expanded_bones_and_physical_spawn_height() {
        let config = bestiary_sim::config::Config::bundled();
        for (name, genome) in bestiary_core::genome::zoo(&config.genome) {
            let body = genome.expand(&config.genome).unwrap();
            let poses = body_rest_poses(&body);
            let lift = bestiary_sim::genome::spawn_lift(&body);
            assert_eq!(poses.len(), body.parts.len(), "{name}");
            for (part, pose) in body.parts.iter().zip(&poses) {
                let expected = part.pose.translation.vector;
                assert_eq!(
                    pose.translation,
                    [expected.x, expected.y + lift, expected.z]
                );
                assert_eq!(pose.rotation, <[f32; 4]>::from(part.pose.rotation.coords));
            }
        }
    }
    #[test]
    fn planted_foot_and_muscle_attachment_remain_fixed_as_torso_advances() {
        let foot = Pose {
            translation: [1.0, 0.08, 2.0],
            rotation: Quat::from_rotation_z(0.3).to_array(),
        };
        let local = Vec3::new(0.0, -0.08, 0.0);
        let roots = [[0.0, 0.7, 0.0], [0.6, 0.72, 0.3]];
        let a = displayed_point(&foot, local, roots[0], Vec3::ZERO, true);
        let b = displayed_point(&foot, local, roots[1], Vec3::ZERO, true);
        assert_eq!(a, b);
        assert_eq!(a, pose_point(&foot, local));
        let world = displayed_point(&foot, local, roots[1], Vec3::new(10.0, 3.0, 10.0), false);
        assert!((world - (a + Vec3::new(9.4, 3.0, 9.7))).length() < 1e-6);
    }

    #[test]
    fn strike_playback_uses_the_recorded_dynamic_target_after_body_parts() {
        let config = bestiary_sim::config::Config::bundled();
        let genome = bestiary_core::genome::Genome::starter(&config.genome);
        let parts = genome.expand(&config.genome).unwrap().parts.len();
        let trial = bestiary_sim::combine::trial(&genome, &config, TrialKind::Strike, 0.0).unwrap();
        let mut replay = trial.replay.unwrap();
        assert_eq!(replay.frames[0].len(), parts + 1);
        for frame in [0, replay.frames.len() - 1] {
            assert_eq!(
                strike_target_pose(&replay, frame, parts),
                replay.frames[frame].get(parts)
            );
        }
        assert_eq!(
            strike_target_pose(&replay, replay.frames.len(), parts),
            None
        );
        replay.kind = TrialKind::Sprint;
        assert_eq!(strike_target_pose(&replay, 0, parts), None);
        replay.kind = TrialKind::Strike;
        replay.frames[0].truncate(parts);
        assert_eq!(strike_target_pose(&replay, 0, parts), None);
    }

    #[test]
    fn field_overlays_preserve_underwater_values_and_biomass_uses_nutrients() {
        let mut cell = bestiary_world::Cell {
            height: -1.0,
            temperature: 0.0,
            moisture: 0.0,
            soil: 0,
            plant: bestiary_world::Plant {
                energy: 100,
                nutrients: 0,
                regrowth: 0.0,
                toughness: 0.0,
                toxicity: 0.0,
                dispersal: 0,
            },
            pheromones: [0.0; 4],
        };
        for overlay in [
            Overlay::Temperature,
            Overlay::Moisture,
            Overlay::Pheromone(2),
            Overlay::Biomass,
        ] {
            let before = terrain_color(&cell, overlay, true, 100);
            match overlay {
                Overlay::Temperature => cell.temperature = 1.0,
                Overlay::Moisture => cell.moisture = 1.0,
                Overlay::Pheromone(i) => cell.pheromones[i] = 10.0,
                Overlay::Biomass => cell.plant.nutrients = 100,
                _ => unreachable!(),
            }
            let after = terrain_color(&cell, overlay, true, 100);
            assert_ne!(
                before, after,
                "{overlay:?} must expose changing underwater data"
            );
            assert_eq!(after, terrain_color(&cell, overlay, false, 100));
        }
        let biomass = terrain_color(&cell, Overlay::Biomass, true, 100);
        cell.plant.energy *= 1000;
        assert_eq!(biomass, terrain_color(&cell, Overlay::Biomass, true, 100));
    }

    #[test]
    fn predation_overlay_uses_recent_actual_attacker_and_victim_ids() {
        use bestiary_world::EcologyEvent;
        let events = vec![
            (
                90,
                EcologyEvent::Predation {
                    attacker: 1,
                    victim: 2,
                },
            ),
            (
                91,
                EcologyEvent::Predation {
                    attacker: 3,
                    victim: 4,
                },
            ),
            (
                100,
                EcologyEvent::Predation {
                    attacker: 4,
                    victim: 5,
                },
            ),
            (
                100,
                EcologyEvent::RecordLifespan {
                    creature: 6,
                    age: 10,
                },
            ),
            (
                101,
                EcologyEvent::Predation {
                    attacker: 7,
                    victim: 8,
                },
            ),
        ];
        assert_eq!(
            recent_predation(&events, 100, 10),
            BTreeSet::from([3, 4, 5])
        );
        assert_eq!(recent_predation(&events, 100, 1), BTreeSet::from([4, 5]));
        assert!(recent_predation(&events, 50, 10).is_empty());
    }

    #[test]
    fn clip_locks_native_window_dimensions_and_restores_original_setting() {
        for was_resizable in [false, true] {
            let mut resizable = was_resizable;
            let mut original = None;
            for _ in 0..3 {
                clip_window_resizing(true, &mut resizable, &mut original);
                assert!(!resizable);
                assert_eq!(original, Some(was_resizable));
            }
            clip_window_resizing(false, &mut resizable, &mut original);
            assert_eq!(resizable, was_resizable);
            assert_eq!(original, None);
        }
    }
    #[test]
    fn anatomy_covers_zoo_joints_muscles_and_terminal_foot_contact_without_changing_genomes() {
        let config = bestiary_sim::config::Config::bundled();
        let appearance =
            anatomy::Appearance::parse(include_str!("../../../config/viewer.ron")).unwrap();
        for (_, genome) in bestiary_core::genome::zoo(&config.genome) {
            let hash = genome.hash();
            let body = genome.expand(&config.genome).unwrap();
            for anatomy in [true, false] {
                let mut world = bevy::ecs::world::World::new();
                let mut queue = bevy::ecs::world::CommandQueue::default();
                let mut meshes = Assets::<Mesh>::default();
                let mut materials = Assets::<StandardMaterial>::default();
                let mut palette = Palette {
                    cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
                    capsule: meshes.add(Capsule3d::new(0.5, 0.0)),
                    proxy: meshes.add(anatomy::proxy_mesh(&appearance)),
                    bone: meshes.add(anatomy::tissue_mesh(&appearance, false)),
                    torso: meshes.add(anatomy::torso_mesh(&appearance)),
                    muscle: meshes.add(anatomy::tissue_mesh(&appearance, true)),
                    colors: BTreeMap::new(),
                    body_meshes: BTreeMap::new(),
                    muscle_materials: BTreeMap::new(),
                };
                draw_body(
                    &mut Commands::new(&mut queue, &world),
                    &mut palette,
                    &mut materials,
                    &mut meshes,
                    hash,
                    &body,
                    &genome,
                    Vec3::ZERO,
                    PoseSource::Inspector,
                    None,
                    anatomy,
                    &appearance,
                );
                queue.apply(&mut world);
                assert_eq!(
                    world.query::<&BodyPart>().iter(&world).count(),
                    if anatomy {
                        body.parts.len() * 2 - 1
                    } else {
                        body.parts.len()
                    }
                );
                assert_eq!(
                    world.query::<&MuscleVisual>().iter(&world).count(),
                    if anatomy { body.muscles.len() } else { 0 }
                );
                for (m, transform) in world.query::<(&MuscleVisual, &Transform)>().iter(&world) {
                    let origin = pose_point(&m.bases[0], m.origin);
                    let insertion = pose_point(&m.bases[1], m.insertion);
                    assert!(transform.transform_point(Vec3::Y * -0.5).distance(origin) < 1e-5);
                    assert!(transform.transform_point(Vec3::Y * 0.5).distance(insertion) < 1e-5);
                }
                for (visual, mesh, transform) in world
                    .query::<(&BodyPart, &Mesh3d, &Transform)>()
                    .iter(&world)
                {
                    let part = &body.parts[visual.index];
                    if visual.local_translation != Vec3::ZERO
                        || part.parent.is_none()
                        || body
                            .parts
                            .iter()
                            .any(|child| child.parent == Some(visual.index))
                    {
                        continue;
                    }
                    assert_eq!(visual.local_rotation, Quat::IDENTITY);
                    let bevy::render::mesh::VertexAttributeValues::Float32x3(vertices) = meshes
                        .get(&mesh.0)
                        .unwrap()
                        .attribute(Mesh::ATTRIBUTE_POSITION)
                        .unwrap()
                    else {
                        panic!("expected positions");
                    };
                    // Tilting a planted foot onto an edge must retain the same
                    // contact height as its actual box or capsule collider.
                    for tilt in [
                        Quat::IDENTITY,
                        Quat::from_euler(EulerRot::XYZ, 0.35, 0.2, -0.6),
                    ] {
                        let support = vertices
                            .iter()
                            .map(|p| (tilt * (Vec3::from_array(*p) * transform.scale)).y)
                            .fold(f32::INFINITY, f32::min);
                        let (expected, tolerance) = match part.shape {
                            Shape::Box => {
                                let axes = Mat3::from_quat(tilt);
                                let extent = axes.x_axis.y.abs() * part.size[0]
                                    + axes.y_axis.y.abs() * part.size[1]
                                    + axes.z_axis.y.abs() * part.size[2];
                                (-extent * 0.5, 1e-5)
                            }
                            Shape::Capsule => {
                                let radius = part.size[0].min(part.size[2]) * 0.5;
                                let half_segment = (part.size[1] * 0.5 - radius).max(0.0);
                                (
                                    -radius - half_segment * (tilt * Vec3::Y).y.abs(),
                                    radius * 0.02 + 1e-5,
                                )
                            }
                        };
                        assert!((support - expected).abs() <= tolerance,
                            "terminal part {} rendered support {support} differs from physical support {expected}", visual.index);
                    }
                }
                assert_eq!(genome.hash(), hash);
            }
        }
    }
    #[test]
    fn retina_viewport_stays_inside_render_target_after_panel_resize() {
        let maximum = UVec2::new(2880, 1920);
        let (position, size) = viewport_bounds([255.0, 48.0, 1185.0, 912.0], 2.0, maximum);
        assert_eq!(position, UVec2::new(510, 96));
        assert_eq!(size, UVec2::new(2370, 1824));
        for rect in [[255.0, 48.0, 1440.0, 960.0], [3000.0; 4], [-1.0; 4]] {
            let (position, size) = viewport_bounds(rect, 2.0, maximum);
            assert!((position + size).cmple(maximum).all());
            assert!(size.cmpge(UVec2::ONE).all());
        }
    }
    #[test]
    fn offline_camera_movement_selects_new_near_bodies_with_stable_ties() {
        let mut world =
            bestiary_world::World::new(42, bestiary_sim::config::Config::bundled(), 3).unwrap();
        world.agents[0].position = [0.0, 0.0];
        world.agents[1].position = [0.0, 0.0];
        world.agents[2].position = [100.0, 100.0];
        assert_eq!(
            nearest_bodies(&world, Vec3::ZERO, 1),
            BTreeSet::from([world.agents[0].id])
        );
        assert_eq!(
            nearest_bodies(&world, Vec3::new(100.0, 0.0, 100.0), 1),
            BTreeSet::from([world.agents[2].id])
        );
        assert!(nearest_bodies(&world, Vec3::ZERO, 0).is_empty());
    }
}
