//! ECS resources and components: the simulation's shared state and the tags
//! attached to spawned entities.

use bevy::prelude::*;

use crate::physics::SimState;

/// The state currently on screen.
#[derive(Resource)]
pub(crate) struct CurrentState(pub(crate) SimState);

/// The simulation clock that drives time evolution of superpositions.
#[derive(Resource)]
pub(crate) struct SimClock {
    pub(crate) t: f32,
    pub(crate) speed: f32,
    pub(crate) paused: bool,
}

/// Normalisation for the instantaneous density so point scale stays in range.
#[derive(Resource)]
pub(crate) struct DensityRef(pub(crate) f32);

/// Orbit-camera parameters, driven each frame by `drive_camera`.
#[derive(Resource)]
pub(crate) struct CameraRig {
    pub(crate) angle: f32,
    pub(crate) distance: f32,
    pub(crate) height: f32,
    pub(crate) auto_rotate: bool,
}

/// Shared handles created once at startup and reused for every point.
#[derive(Resource)]
pub(crate) struct CloudAssets {
    pub(crate) point_mesh: Handle<Mesh>,
    pub(crate) positive: Handle<StandardMaterial>,
    pub(crate) negative: Handle<StandardMaterial>,
    pub(crate) hues: Vec<Handle<StandardMaterial>>,
}

/// A cloud point, remembering the fixed position it samples the field at.
#[derive(Component)]
pub(crate) struct CloudPoint {
    pub(crate) home: Vec3,
}

/// Marks points that are updated every frame (superposition mode).
#[derive(Component)]
pub(crate) struct Animated;

/// The hue bucket a point currently uses, so we only swap materials on change.
#[derive(Component)]
pub(crate) struct HueBucket(pub(crate) usize);

/// Roles for the pieces of dynamic on-screen text.
#[derive(Component)]
pub(crate) enum TextRole {
    Title,
    Subtitle,
}

/// Container whose children make up the colour legend (rebuilt on change).
#[derive(Component)]
pub(crate) struct LegendRoot;
