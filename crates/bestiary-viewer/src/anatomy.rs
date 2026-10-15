//! Procedural presentation of existing bones and straight-line muscles.
//! No rendered tissue adds collision geometry or changes simulation state.
use bevy::{
    prelude::*,
    render::{mesh::Indices, render_asset::RenderAssetUsages, render_resource::PrimitiveTopology},
};
use serde::Deserialize;

#[derive(Resource, Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Appearance {
    pub muscle_radius_scale: f32,
    pub muscle_max_radius_fraction: f32,
    pub muscle_taper_power: f32,
    pub stretch_min: f32,
    pub stretch_max: f32,
    pub joint_radius_fraction: f32,
    pub radial_segments: usize,
    pub bone_rings: usize,
    pub bone_waist: f32,
    pub torso_rounding: f32,
    pub bone_color: [f32; 3],
    pub gene_tint: f32,
    pub muscle_color: [f32; 3],
    pub tendon_color: [f32; 3],
    pub active_color: [f32; 3],
    pub ground_color: [f32; 3],
    pub grid_color: [f32; 3],
    pub contact_grid_spacing: f32,
    pub contact_grid_radius: f32,
    pub inspector_shadows: bool,
}
impl Appearance {
    pub fn parse(source: &str) -> Result<Self, String> {
        let a: Self = ron::from_str(source).map_err(|e| e.to_string())?;
        if !a.muscle_radius_scale.is_finite()
            || !(0.01..=1.0).contains(&a.muscle_radius_scale)
            || !(0.01..=0.25).contains(&a.muscle_max_radius_fraction)
            || !(1.0..=4.0).contains(&a.muscle_taper_power)
            || !a.stretch_min.is_finite()
            || !(0.1..=1.0).contains(&a.stretch_min)
            || !a.stretch_max.is_finite()
            || !(1.0..=10.0).contains(&a.stretch_max)
            || !a.joint_radius_fraction.is_finite()
            || !(0.1..=1.0).contains(&a.joint_radius_fraction)
            || !(6..=32).contains(&a.radial_segments)
            || !(8..=32).contains(&a.bone_rings)
            || !(0.1..=1.0).contains(&a.bone_waist)
            || !(0.01..=0.5).contains(&a.torso_rounding)
            || !(0.0..=1.0).contains(&a.gene_tint)
            || !(0.05..=2.0).contains(&a.contact_grid_spacing)
            || !(1.0..=20.0).contains(&a.contact_grid_radius)
            || [
                a.bone_color,
                a.muscle_color,
                a.tendon_color,
                a.active_color,
                a.ground_color,
                a.grid_color,
            ]
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err("invalid viewer anatomy settings".into());
        }
        Ok(a)
    }
    pub fn bone_tint(&self, gene: [f32; 3]) -> [f32; 3] {
        std::array::from_fn(|i| {
            self.bone_color[i] * (1.0 - self.gene_tint) + gene[i] * self.gene_tint
        })
    }
    /// Contraction modulates reflected pigment; live muscle has no light source.
    pub fn muscle_material(&self, activation: f32) -> StandardMaterial {
        let activation = if activation.is_finite() {
            activation.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let tint = self
            .active_color
            .map(|value| 1.0 + (value - 1.0) * activation);
        StandardMaterial {
            base_color: Color::srgb(tint[0], tint[1], tint[2]),
            perceptual_roughness: 0.9,
            ..default()
        }
    }
    pub fn muscle_tint(&self, activation: f32) -> Color {
        let pigment = Color::srgb(
            self.muscle_color[0],
            self.muscle_color[1],
            self.muscle_color[2],
        )
        .to_linear();
        let multiplier = self.muscle_material(activation).base_color.to_linear();
        Color::linear_rgb(
            pigment.red * multiplier.red,
            pigment.green * multiplier.green,
            pigment.blue * multiplier.blue,
        )
    }
}

/// Closed surface of revolution, with elliptical scaling applied by the caller.
/// The muscle profile includes narrow tendon ends and a fusiform belly.
pub fn tissue_mesh(a: &Appearance, muscle: bool) -> Mesh {
    let rings = a.bone_rings;
    let sides = a.radial_segments;
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();
    for ring in 0..=rings {
        let t = ring as f32 / rings as f32;
        let envelope = (std::f32::consts::PI * t).sin().max(0.0);
        let radius = if muscle {
            envelope.powf(a.muscle_taper_power)
        } else {
            // A narrow shaft with rounded epiphyses; maximum diameter is one.
            0.5 * envelope.sqrt() * (a.bone_waist + (1.0 - a.bone_waist) * (2.0 * t - 1.0).powi(2))
        };
        for side in 0..=sides {
            let angle = std::f32::consts::TAU * side as f32 / sides as f32;
            positions.push([radius * angle.cos(), t - 0.5, radius * angle.sin()]);
            let belly = ((t.min(1.0 - t) - 0.10) / 0.14).clamp(0.0, 1.0);
            let rgb = if muscle {
                std::array::from_fn(|i| {
                    a.tendon_color[i] * (1.0 - belly) + a.muscle_color[i] * belly
                })
            } else {
                [1.0; 3]
            };
            // Bevy vertex colors are linear, while the editable pigments are sRGB.
            let linear = Color::srgb(rgb[0], rgb[1], rgb[2]).to_linear();
            colors.push([linear.red, linear.green, linear.blue, 1.0]);
            if ring < rings && side < sides {
                let n = (ring * (sides + 1) + side) as u32;
                let next = n + (sides + 1) as u32;
                indices.extend([n, next, n + 1, n + 1, next, next + 1]);
            }
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices));
    mesh.compute_smooth_normals();
    mesh
}

/// Bevel a root box inside its original bounds, retaining the joint-bearing torso.
pub fn torso_mesh(a: &Appearance) -> Mesh {
    let steps = a.radial_segments;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    for (normal, u, v) in [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (Vec3::Y, Vec3::Z, Vec3::X),
        (Vec3::Z, Vec3::X, Vec3::Y),
    ] {
        for sign in [-1.0, 1.0] {
            let offset = positions.len() as u32;
            for row in 0..=steps {
                for col in 0..=steps {
                    let point = normal * sign * 0.5
                        + u * (col as f32 / steps as f32 - 0.5)
                        + v * sign * (row as f32 / steps as f32 - 0.5);
                    let inner = point.clamp(
                        Vec3::splat(-0.5 + a.torso_rounding),
                        Vec3::splat(0.5 - a.torso_rounding),
                    );
                    let n = (point - inner).normalize();
                    positions.push((inner + n * a.torso_rounding).to_array());
                    normals.push(n.to_array());
                    if row < steps && col < steps {
                        let i = offset + (row * (steps + 1) + col) as u32;
                        let next = i + (steps + 1) as u32;
                        indices.extend([i, i + 1, next, i + 1, next + 1, next]);
                    }
                }
            }
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}

/// Orient a unit-length Y-axis tissue mesh between actual attachment points.
/// Keeping r²L constant makes shortening visibly thicken a muscle.
pub fn muscle_transform(
    a: Vec3,
    b: Vec3,
    rest: f32,
    area: f32,
    style: &Appearance,
) -> Option<Transform> {
    let delta = b - a;
    let length = delta.length();
    if !length.is_finite()
        || length <= 1e-6
        || !rest.is_finite()
        || rest <= 0.0
        || !area.is_finite()
        || area <= 0.0
    {
        return None;
    }
    let stretch = (length / rest).clamp(style.stretch_min, style.stretch_max);
    let radius = ((area / std::f32::consts::PI / stretch).sqrt() * style.muscle_radius_scale)
        .min(length * style.muscle_max_radius_fraction);
    Some(Transform {
        translation: (a + b) * 0.5,
        rotation: Quat::from_rotation_arc(Vec3::Y, delta / length),
        scale: Vec3::new(radius, length, radius),
    })
}

/// Align a procedural bone's long axis with its gene's longest dimension.
pub fn bone_frame(size: [f32; 3]) -> (Quat, Vec3) {
    let [x, y, z] = size;
    if x > y && x >= z {
        (
            Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2),
            Vec3::new(y, x, z),
        )
    } else if z > y && z > x {
        (
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            Vec3::new(x, z, y),
        )
    } else {
        (Quat::IDENTITY, Vec3::from_array(size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settings() -> Appearance {
        Appearance::parse(include_str!("../../../config/viewer.ron")).unwrap()
    }
    #[test]
    fn muscle_attachments_and_volume_follow_recorded_pose_without_invented_motion() {
        let style = settings();
        for direction in [Vec3::Y, -Vec3::Y, Vec3::new(1.0, 2.0, -3.0).normalize()] {
            let origin = Vec3::new(3.0, 4.0, 5.0);
            let end = origin + direction * 2.0;
            let full = muscle_transform(origin, end, 2.0, 0.1, &style).unwrap();
            assert!(full.transform_point(Vec3::Y * -0.5).distance(origin) < 1e-5);
            assert!(full.transform_point(Vec3::Y * 0.5).distance(end) < 1e-5);
            let short = muscle_transform(origin, origin + direction, 2.0, 0.1, &style).unwrap();
            assert!(short.scale.x > full.scale.x);
            assert!(
                (short.scale.x.powi(2) * short.scale.y - full.scale.x.powi(2) * full.scale.y).abs()
                    < 1e-6
            );
        }
        assert!(muscle_transform(Vec3::ZERO, Vec3::ZERO, 1.0, 0.1, &style).is_none());
        assert!(muscle_transform(Vec3::ZERO, Vec3::Y, 1.0, f32::NAN, &style).is_none());
    }
    #[test]
    fn short_muscles_stay_slender_and_preserve_actual_attachments() {
        let style = settings();
        for length in [0.0001, 0.01, 0.1, 1.0] {
            let end = Vec3::new(length, 0.0, 0.0);
            let transform = muscle_transform(Vec3::ZERO, end, 1.0, 100.0, &style).unwrap();
            assert!(transform.scale.x <= length * style.muscle_max_radius_fraction);
            assert!(transform.transform_point(-Vec3::Y * 0.5).length() < 1e-5);
            assert!(transform.transform_point(Vec3::Y * 0.5).distance(end) < 1e-5);
        }
    }
    #[test]
    fn muscle_activation_changes_reflected_pigment_without_emissive_glow() {
        let style = settings();
        let rest = style.muscle_tint(0.0).to_linear();
        for activation in [f32::NAN, -1.0, 0.0, 0.25, 0.5, 1.0, 2.0] {
            let material = style.muscle_material(activation);
            assert_eq!(material.emissive, LinearRgba::BLACK);
            let tint = style.muscle_tint(activation).to_linear();
            for (value, base) in [
                (tint.red, rest.red),
                (tint.green, rest.green),
                (tint.blue, rest.blue),
            ] {
                assert!(value.is_finite() && value > 0.0 && value <= base);
                assert!(value >= base * 0.6);
            }
            assert!(tint.green >= tint.red && tint.red > tint.blue);
        }
        assert_ne!(style.muscle_tint(0.0), style.muscle_tint(1.0));
    }
    #[test]
    fn anatomy_pigments_use_linear_vertex_colors_and_cohesive_muted_palette() {
        let style = settings();
        let mesh = tissue_mesh(&style, true);
        let bevy::render::mesh::VertexAttributeValues::Float32x4(colors) =
            mesh.attribute(Mesh::ATTRIBUTE_COLOR).unwrap()
        else {
            panic!("expected colors");
        };
        let belly = colors[(style.bone_rings / 2) * (style.radial_segments + 1)];
        let expected = Color::srgb(
            style.muscle_color[0],
            style.muscle_color[1],
            style.muscle_color[2],
        )
        .to_linear();
        assert_eq!(belly, [expected.red, expected.green, expected.blue, 1.0]);
        for pigment in [style.bone_color, style.muscle_color, style.tendon_color] {
            let min = pigment.into_iter().fold(f32::INFINITY, f32::min);
            let max = pigment.into_iter().fold(f32::NEG_INFINITY, f32::max);
            assert!(max - min < 0.2, "tissue palette should be restrained");
        }
    }
    #[test]
    fn procedural_anatomy_has_finite_normals_and_valid_topology() {
        for mesh in [
            tissue_mesh(&settings(), true),
            tissue_mesh(&settings(), false),
            torso_mesh(&settings()),
        ] {
            for attribute in [Mesh::ATTRIBUTE_POSITION, Mesh::ATTRIBUTE_NORMAL] {
                let bevy::render::mesh::VertexAttributeValues::Float32x3(values) =
                    mesh.attribute(attribute).unwrap()
                else {
                    panic!("expected vectors")
                };
                assert!(values.iter().flatten().all(|v| v.is_finite()));
            }
            assert!(mesh
                .indices()
                .unwrap()
                .iter()
                .all(|i| i < mesh.count_vertices()));
        }
    }
    #[test]
    fn bone_frame_preserves_each_genome_axis() {
        for size in [[3.0, 1.0, 2.0], [1.0, 3.0, 2.0], [1.0, 2.0, 3.0]] {
            let (rotation, scale) = bone_frame(size);
            assert!(((rotation * scale).abs() - Vec3::from_array(size)).length() < 1e-5);
        }
    }
    #[test]
    fn viewer_config_rejects_invalid_and_unknown_fields() {
        let source = include_str!("../../../config/viewer.ron");
        for (from, to) in [
            ("muscle_radius_scale: 0.13", "muscle_radius_scale: NaN"),
            (
                "muscle_max_radius_fraction: 0.1",
                "muscle_max_radius_fraction: 1.0",
            ),
            ("muscle_taper_power: 1.5", "muscle_taper_power: NaN"),
            ("stretch_min: 0.4", "stretch_min: 0.0"),
            ("stretch_max: 2.5", "stretch_max: 0.5"),
            ("joint_radius_fraction: 0.4", "joint_radius_fraction: -1.0"),
            ("16,", "10000,"),
            ("20,", "0,"),
            ("gene_tint: 0.12", "gene_tint: 2.0"),
            ("bone_waist: 0.72", "bone_waist: 0.0"),
            ("torso_rounding: 0.16", "torso_rounding: 1.0"),
            ("bone_color: (0.58", "bone_color: (-1.0"),
            ("ground_color: (0.18", "ground_color: (NaN"),
            ("grid_color: (0.28", "grid_color: (2.0"),
            ("contact_grid_spacing: 0.25", "contact_grid_spacing: 0.0"),
            ("contact_grid_radius: 4.0", "contact_grid_radius: NaN"),
            ("gene_tint:", "unknown:"),
        ] {
            assert!(
                Appearance::parse(&source.replace(from, to)).is_err(),
                "{from} -> {to}"
            );
        }
    }
}
