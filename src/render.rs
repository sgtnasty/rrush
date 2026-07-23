//! One-time scene construction and the colour palette shared across the app.

use bevy::prelude::*;
use bevy::text::FontSize;

use crate::components::{CloudAssets, LegendRoot, TextRole};
use crate::{HUES, NUCLEUS_RADIUS, POINT_RADIUS};

/// Build the static scene: shared point assets, the nucleus, lighting, the
/// camera, and the fixed UI overlays (controls, state label, legend host).
pub(crate) fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let point_mesh = meshes.add(Sphere::new(POINT_RADIUS).mesh().ico(1).unwrap());
    let positive = materials.add(emissive_material(positive_color(), 0.35));
    let negative = materials.add(emissive_material(negative_color(), 0.35));

    // Palette around the colour wheel for phase colouring.
    let hues = (0..HUES)
        .map(|i| materials.add(emissive_material(hue_color(i), 0.9)))
        .collect();

    commands.insert_resource(CloudAssets {
        point_mesh,
        positive,
        negative,
        hues,
    });

    // Nucleus (proton) at the origin.
    let nucleus_mesh = meshes.add(Sphere::new(NUCLEUS_RADIUS).mesh().ico(4).unwrap());
    let nucleus_mat = materials.add(emissive_material(Color::srgb(1.0, 0.85, 0.2), 1.4));
    commands.spawn((
        Mesh3d(nucleus_mesh),
        MeshMaterial3d(nucleus_mat),
        Transform::from_translation(Vec3::ZERO),
    ));

    // A light so the far side of the cloud still has some shading.
    commands.spawn((
        PointLight {
            shadow_maps_enabled: false,
            intensity: 5_000_000.0,
            range: 200.0,
            ..default()
        },
        Transform::from_xyz(20.0, 30.0, 20.0),
    ));

    // Camera. Its transform is driven every frame by `drive_camera`.
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 4.0, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
        AmbientLight {
            brightness: 220.0,
            ..default()
        },
    ));

    // Fixed controls panel (top-left).
    commands.spawn((
        Text::new(
            "[1] 1s  [2] 2s  [3] 2p_z  [4] 3p_z  [5] 3d_z2  [6] 3d_xy\n\
             [7] 1s+2p_z  [8] 1s+2s  [9] 2p_z+3d_z2  [0] 1s+2p_z+3d_z2\n\
             [R] resample  [Space] spin  [P] pause\n\
             [ [ ] ] time speed   arrows: orbit / zoom   [Q] quit",
        ),
        TextFont {
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::srgb(0.7, 0.75, 0.85)),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            ..default()
        },
    ));

    // Prominent state label (top-centre): big name + a status subtitle.
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: px(10),
            width: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(2),
            ..default()
        })
        .with_children(|bar| {
            bar.spawn((
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(30.0),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.97, 1.0)),
                TextRole::Title,
            ));
            bar.spawn((
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(Color::srgb(0.65, 0.72, 0.85)),
                TextRole::Subtitle,
            ));
        });

    // Colour legend container (bottom-left), filled in by `update_legend`.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: px(14),
            left: px(14),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            ..default()
        },
        LegendRoot,
    ));
}

/// Colour of points where ψ is positive.
pub(crate) fn positive_color() -> Color {
    Color::srgb(0.25, 0.55, 1.0)
}

/// Colour of points where ψ is negative.
pub(crate) fn negative_color() -> Color {
    Color::srgb(1.0, 0.45, 0.35)
}

/// Colour of phase bucket `i` on the hue wheel.
pub(crate) fn hue_color(i: usize) -> Color {
    Color::hsl(i as f32 / HUES as f32 * 360.0, 0.85, 0.55)
}

/// A `StandardMaterial` that both reflects and glows in the given colour.
fn emissive_material(color: Color, glow: f32) -> StandardMaterial {
    let lin = color.to_linear();
    StandardMaterial {
        base_color: color,
        emissive: LinearRgba::rgb(lin.red * glow, lin.green * glow, lin.blue * glow),
        ..default()
    }
}
