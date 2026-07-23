//! rrush — a real-time visualisation of the hydrogen atom.
//!
//! The proton (nucleus) sits at the origin. Around it a Monte-Carlo point
//! cloud represents the electron.
//!
//! * Single orbitals (keys 1..6) are energy eigenstates. Their probability
//!   density |ψ|² is stationary — only an unobservable global phase turns —
//!   so they are drawn as a static cloud, coloured by the sign of ψ.
//!
//! * Superpositions of states with *different* energies (keys 7..0) are not
//!   stationary: the cross terms make the charge density slosh at the Bohr
//!   frequencies ω = (Eₙ − Eₘ)/ħ. These are animated — each point's size
//!   tracks the instantaneous |Ψ(r,t)|² and its colour tracks the local
//!   phase arg(Ψ), so you can watch the electron density move.
//!
//! Controls
//!   1..6      single orbital: 1s, 2s, 2p_z, 3p_z, 3d_z², 3d_xy
//!   7..0      superposition: 1s+2p_z, 1s+2s, 2p_z+3d_z², 1s+2p_z+3d_z²
//!   R         resample the current cloud
//!   Space     toggle auto-rotation
//!   P         pause / resume time
//!   [ / ]     slow down / speed up time
//!   ← / →     orbit the camera
//!   ↑ / ↓     zoom in / out
//!   Q         quit
//!
//! The code is split into focused modules:
//!   * [`physics`]    — orbitals and the (Bevy-free) quantum state, with tests
//!   * [`components`] — ECS resources and component tags
//!   * [`render`]     — one-time scene setup and the colour palette
//!   * [`systems`]    — the per-frame update systems
//!   * [`screenshot`] — scripted stills / GIF frames (`RRUSH_SHOT`)
//!   * [`util`]       — small stateless helpers

mod components;
mod physics;
mod render;
mod screenshot;
mod systems;
mod util;

use bevy::prelude::*;

use components::{CameraRig, CurrentState, DensityRef, SimClock, SpeedOverride};
use physics::{Orbital, SimState};
use screenshot::shot_plan_from_env;
use util::state_from_key;

/// Number of points drawn for the electron cloud.
pub(crate) const CLOUD_POINTS: usize = 4200;
/// Radius of each little cloud sphere, in atomic units (Bohr radii).
pub(crate) const POINT_RADIUS: f32 = 0.11;
/// Radius of the nucleus sphere.
pub(crate) const NUCLEUS_RADIUS: f32 = 0.35;
/// Number of hues used to colour the phase of a superposition.
pub(crate) const HUES: usize = 24;

fn main() {
    // Optional: `rrush <state>` opens directly in a state (1..6 single
    // orbitals, 7..0 superposition presets). `RRUSH_STILL=1` starts without
    // auto-rotation for a steady view.
    let initial = std::env::args().nth(1).and_then(|a| state_from_key(&a));
    let auto_rotate = std::env::var("RRUSH_STILL").is_err();
    let initial = initial.unwrap_or(SimState::Single(Orbital::S1));
    let distance = initial.view_radius() * 2.4;
    let speed = if initial.is_dynamic() {
        initial.suggested_speed()
    } else {
        4.0
    };
    // `RRUSH_SPEED` overrides the animation speed (handy for slow, smooth
    // frame capture when generating documentation GIFs).
    let speed_override = std::env::var("RRUSH_SPEED")
        .ok()
        .and_then(|s| s.parse::<f32>().ok());
    let speed = speed_override.unwrap_or(speed);
    let shot_plan = shot_plan_from_env();

    let mut app = App::new();
    if std::env::var("RRUSH_FPS").is_ok() {
        app.add_plugins((
            bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
            bevy::diagnostic::LogDiagnosticsPlugin::default(),
        ));
    }
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "rrush — hydrogen atom".into(),
            ..default()
        }),
        ..default()
    }))
    .insert_resource(ClearColor(Color::srgb(0.02, 0.02, 0.05)))
    .insert_resource(CurrentState(initial))
    .insert_resource(SimClock {
        t: 0.0,
        speed,
        paused: false,
    })
    .insert_resource(DensityRef(1.0))
    .insert_resource(SpeedOverride(speed_override))
    .insert_resource(CameraRig {
        // A side-on angle (still mode) shows z-axis motion across the view.
        angle: if auto_rotate { 0.0 } else { 1.0 },
        distance,
        height: 0.35,
        auto_rotate,
    })
    .add_systems(Startup, render::setup)
    .add_systems(
        Update,
        (
            systems::handle_input,
            systems::spawn_cloud,
            systems::evolve,
            systems::update_labels,
            systems::update_legend,
            systems::drive_camera,
            screenshot::capture_screenshots,
        )
            .chain(),
    );

    if let Some(plan) = shot_plan {
        app.insert_resource(plan);
    }

    app.run();
}
