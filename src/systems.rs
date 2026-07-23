//! The per-frame systems: input handling, cloud (re)sampling, time evolution,
//! and the label/legend/camera updates.

use bevy::prelude::*;
use bevy::text::FontSize;
use rand::Rng;

use crate::components::{
    Animated, CameraRig, CloudAssets, CloudPoint, CurrentState, DensityRef, HueBucket, LegendRoot,
    SimClock, TextRole,
};
use crate::render::{hue_color, negative_color, positive_color};
use crate::util::{hue_bucket, point_scale, sample_cube, state_from_key};
use crate::{CLOUD_POINTS, HUES};

/// Keyboard controls: pick a state, steer the camera, and drive the clock.
pub(crate) fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut current: ResMut<CurrentState>,
    mut clock: ResMut<SimClock>,
    mut rig: ResMut<CameraRig>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::KeyQ) {
        exit.write(AppExit::Success);
        return;
    }

    let digits = [
        (KeyCode::Digit1, "1"),
        (KeyCode::Digit2, "2"),
        (KeyCode::Digit3, "3"),
        (KeyCode::Digit4, "4"),
        (KeyCode::Digit5, "5"),
        (KeyCode::Digit6, "6"),
        (KeyCode::Digit7, "7"),
        (KeyCode::Digit8, "8"),
        (KeyCode::Digit9, "9"),
        (KeyCode::Digit0, "0"),
    ];
    for (key, label) in digits {
        if keys.just_pressed(key) {
            if let Some(state) = state_from_key(label) {
                current.0 = state;
            }
        }
    }

    if keys.just_pressed(KeyCode::KeyR) {
        current.set_changed();
    }
    if keys.just_pressed(KeyCode::Space) {
        rig.auto_rotate = !rig.auto_rotate;
    }
    if keys.just_pressed(KeyCode::KeyP) {
        clock.paused = !clock.paused;
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        clock.speed = (clock.speed * 0.66).max(0.25);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        clock.speed = (clock.speed * 1.5).min(40.0);
    }

    let dt = time.delta_secs();
    if keys.pressed(KeyCode::ArrowLeft) {
        rig.angle -= dt * 1.2;
    }
    if keys.pressed(KeyCode::ArrowRight) {
        rig.angle += dt * 1.2;
    }
    if keys.pressed(KeyCode::ArrowUp) {
        rig.distance = (rig.distance - dt * rig.distance).max(1.5);
    }
    if keys.pressed(KeyCode::ArrowDown) {
        rig.distance += dt * rig.distance;
    }
}

/// When the state changes, clear the old cloud and Monte-Carlo sample a fresh
/// set of fixed points from the (time-averaged) envelope density.
pub(crate) fn spawn_cloud(
    mut commands: Commands,
    current: Res<CurrentState>,
    assets: Res<CloudAssets>,
    mut clock: ResMut<SimClock>,
    mut density_ref: ResMut<DensityRef>,
    mut rig: ResMut<CameraRig>,
    existing: Query<Entity, With<CloudPoint>>,
) {
    if !current.is_changed() {
        return;
    }

    for entity in &existing {
        commands.entity(entity).despawn();
    }

    let state = &current.0;
    let dynamic = state.is_dynamic();
    let reach = state.view_radius();
    rig.distance = reach * 2.4;
    if dynamic {
        clock.t = 0.0;
        clock.speed = state.suggested_speed();
    }

    let mut rng = rand::rng();

    // Estimate the peak envelope value for efficient rejection sampling.
    let mut peak = f32::MIN_POSITIVE;
    for _ in 0..40_000 {
        let (x, y, z) = sample_cube(&mut rng, reach);
        peak = peak.max(state.envelope(x, y, z));
    }
    peak *= 1.4;

    // Rejection-sample fixed point positions from the envelope density.
    let mut points: Vec<Vec3> = Vec::with_capacity(CLOUD_POINTS);
    let max_attempts = CLOUD_POINTS * 400;
    let mut attempts = 0;
    while points.len() < CLOUD_POINTS && attempts < max_attempts {
        attempts += 1;
        let (x, y, z) = sample_cube(&mut rng, reach);
        if rng.random::<f32>() * peak < state.envelope(x, y, z) {
            points.push(Vec3::new(x, y, z));
        }
    }

    // Reference density = max over points of (Σ|cₖψₖ|)², an upper bound on
    // the instantaneous |Ψ|², so per-point scale stays within range.
    let mut ref_density = f32::MIN_POSITIVE;
    for p in &points {
        let bound: f32 = state
            .terms()
            .iter()
            .map(|t| (t.coeff * t.orbital.psi(p.x, p.y, p.z)).abs())
            .sum();
        ref_density = ref_density.max(bound * bound);
    }
    density_ref.0 = ref_density.max(1e-12);

    for home in points {
        if dynamic {
            let (re, im) = state.amplitude(home.x, home.y, home.z, clock.t);
            let density = re * re + im * im;
            let bucket = hue_bucket(im.atan2(re));
            commands.spawn((
                Mesh3d(assets.point_mesh.clone()),
                MeshMaterial3d(assets.hues[bucket].clone()),
                Transform::from_translation(home).with_scale(point_scale(density, density_ref.0)),
                CloudPoint { home },
                Animated,
                HueBucket(bucket),
            ));
        } else {
            // Stationary state: colour by the sign of ψ, no time evolution.
            let psi = state.terms()[0].orbital.psi(home.x, home.y, home.z);
            let material = if psi >= 0.0 {
                assets.positive.clone()
            } else {
                assets.negative.clone()
            };
            commands.spawn((
                Mesh3d(assets.point_mesh.clone()),
                MeshMaterial3d(material),
                Transform::from_translation(home),
                CloudPoint { home },
            ));
        }
    }
}

/// Advance the clock and update every animated point from |Ψ(r,t)|² and phase.
pub(crate) fn evolve(
    time: Res<Time>,
    current: Res<CurrentState>,
    assets: Res<CloudAssets>,
    density_ref: Res<DensityRef>,
    mut clock: ResMut<SimClock>,
    mut points: Query<
        (
            &CloudPoint,
            &mut Transform,
            &mut MeshMaterial3d<StandardMaterial>,
            &mut HueBucket,
        ),
        With<Animated>,
    >,
) {
    if !current.0.is_dynamic() || clock.paused {
        return;
    }
    clock.t += time.delta_secs() * clock.speed;

    let state = &current.0;
    let t = clock.t;
    let ref_d = density_ref.0;
    for (point, mut transform, mut material, mut bucket) in &mut points {
        let (re, im) = state.amplitude(point.home.x, point.home.y, point.home.z, t);
        let density = re * re + im * im;
        transform.scale = point_scale(density, ref_d);

        let want = hue_bucket(im.atan2(re));
        if want != bucket.0 {
            bucket.0 = want;
            material.0 = assets.hues[want].clone();
        }
    }
}

/// Refresh the title/subtitle labels when the state or clock changes.
pub(crate) fn update_labels(
    current: Res<CurrentState>,
    clock: Res<SimClock>,
    mut labels: Query<(&mut Text, &TextRole)>,
) {
    if !current.is_changed() && !clock.is_changed() {
        return;
    }
    let subtitle = if current.0.is_dynamic() {
        format!(
            "evolving  |  x{:.1} speed{}",
            clock.speed,
            if clock.paused { "  (paused)" } else { "" }
        )
    } else {
        "stationary state".to_string()
    };
    for (mut text, role) in &mut labels {
        match role {
            TextRole::Title => **text = current.0.name().to_string(),
            TextRole::Subtitle => **text = subtitle.clone(),
        }
    }
}

/// Rebuild the colour legend when the kind of state changes.
pub(crate) fn update_legend(
    mut commands: Commands,
    current: Res<CurrentState>,
    legend: Query<(Entity, Option<&Children>), With<LegendRoot>>,
) {
    if !current.is_changed() {
        return;
    }
    let Ok((root, children)) = legend.single() else {
        return;
    };
    if let Some(children) = children {
        for &child in children {
            commands.entity(child).despawn();
        }
    }

    let heading = |size: f32| TextFont {
        font_size: FontSize::Px(size),
        ..default()
    };
    let dynamic = current.0.is_dynamic();

    commands.entity(root).with_children(|legend| {
        legend.spawn((
            Text::new(if dynamic {
                "colour = phase of Psi"
            } else {
                "colour = sign of Psi"
            }),
            heading(15.0),
            TextColor(Color::srgb(0.82, 0.87, 0.96)),
        ));

        if dynamic {
            // A gradient bar of the hue wheel.
            legend
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    ..default()
                })
                .with_children(|bar| {
                    for i in 0..HUES {
                        bar.spawn((
                            Node {
                                width: px(8),
                                height: px(14),
                                ..default()
                            },
                            BackgroundColor(hue_color(i)),
                        ));
                    }
                });
            legend.spawn((
                Text::new("size = probability |Psi|^2"),
                heading(13.0),
                TextColor(Color::srgb(0.68, 0.73, 0.84)),
            ));
        } else {
            for (color, label) in [
                (positive_color(), "Psi > 0"),
                (negative_color(), "Psi < 0"),
            ] {
                legend
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: px(8),
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((
                            Node {
                                width: px(16),
                                height: px(16),
                                ..default()
                            },
                            BackgroundColor(color),
                        ));
                        row.spawn((
                            Text::new(label),
                            heading(15.0),
                            TextColor(Color::srgb(0.85, 0.88, 0.95)),
                        ));
                    });
            }
        }
    });
}

/// Orbit the camera around the origin.
pub(crate) fn drive_camera(
    time: Res<Time>,
    mut rig: ResMut<CameraRig>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
) {
    if rig.auto_rotate {
        rig.angle += time.delta_secs() * 0.25;
    }
    let Ok(mut transform) = camera.single_mut() else {
        return;
    };
    let x = rig.angle.sin() * rig.distance;
    let z = rig.angle.cos() * rig.distance;
    let y = rig.height * rig.distance;
    *transform = Transform::from_xyz(x, y, z).looking_at(Vec3::ZERO, Vec3::Y);
}
