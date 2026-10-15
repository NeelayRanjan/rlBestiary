//! Render the normal viewer and UI into a GPU texture without a display refresh
//! cap. Wait for each GPU submission so the result measures completed rendering.
use super::*;
use bevy::{
    render::{
        camera::{RenderTarget, ScalingMode},
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, Maintain, TextureDimension, TextureFormat, TextureUsages},
        renderer::RenderDevice,
        view::RenderLayers,
        Render, RenderApp, RenderSet,
    },
    window::PrimaryWindow,
};
use bevy_egui::{EguiContextSettings, EguiRenderToImage};

#[derive(Resource)]
pub(crate) struct Target(pub(crate) Handle<Image>);
pub fn install(app: &mut App) {
    app.add_systems(PreStartup, setup)
        .add_systems(PostStartup, attach);
    app.sub_app_mut(RenderApp)
        .add_systems(Render, finish_gpu.after(RenderSet::Render));
}
fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera_only: Option<Res<export::CameraOnly>>,
) {
    let mut image = Image::new_fill(
        Extent3d {
            width: 2880,
            height: 1920,
            ..default()
        },
        TextureDimension::D2,
        &[0; 4],
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC;
    let ui_image = images.add(image.clone());
    let handle = images.add(image);
    commands.spawn((
        EguiRenderToImage::new(ui_image.clone()),
        EguiContextSettings {
            scale_factor: 2.0,
            ..default()
        },
        PrimaryWindow,
    ));
    if camera_only.is_none() {
        // bevy_egui renders image contexts before CameraDriver. Composite a separate
        // transparent UI texture in a second camera so the scene cannot erase it.
        commands.spawn((
            Mesh3d(meshes.add(Rectangle::new(2880.0, 1920.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color_texture: Some(ui_image),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            })),
            Transform::default(),
            RenderLayers::layer(1),
        ));
        commands.spawn((
            Camera3d::default(),
            Camera {
                order: 1,
                target: RenderTarget::Image(handle.clone()),
                clear_color: ClearColorConfig::None,
                ..default()
            },
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::Fixed {
                    width: 2880.0,
                    height: 1920.0,
                },
                ..OrthographicProjection::default_3d()
            }),
            Transform::from_xyz(0.0, 0.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
            bevy::core_pipeline::tonemapping::Tonemapping::None,
            RenderLayers::layer(1),
        ));
    }
    commands.insert_resource(Target(handle));
}
fn attach(target: Res<Target>, mut cameras: Query<&mut Camera, With<scene::SceneCamera>>) {
    for mut camera in &mut cameras {
        camera.target = RenderTarget::Image(target.0.clone());
    }
}
fn finish_gpu(device: Res<RenderDevice>) {
    device.poll(Maintain::Wait);
}
