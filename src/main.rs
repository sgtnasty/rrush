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

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::text::FontSize;
use rand::Rng;

/// Number of points drawn for the electron cloud.
const CLOUD_POINTS: usize = 4200;
/// Radius of each little cloud sphere, in atomic units (Bohr radii).
const POINT_RADIUS: f32 = 0.11;
/// Radius of the nucleus sphere.
const NUCLEUS_RADIUS: f32 = 0.35;
/// Number of hues used to colour the phase of a superposition.
const HUES: usize = 24;

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
    .insert_resource(CameraRig {
        // A side-on angle (still mode) shows z-axis motion across the view.
        angle: if auto_rotate { 0.0 } else { 1.0 },
        distance,
        height: 0.35,
        auto_rotate,
    })
    .add_systems(Startup, setup)
    .add_systems(
        Update,
        (
            handle_input,
            spawn_cloud,
            evolve,
            update_labels,
            update_legend,
            drive_camera,
            capture_screenshots,
        )
            .chain(),
    );

    if let Some(plan) = shot_plan {
        app.insert_resource(plan);
    }

    app.run();
}

/// Read an optional screenshot plan from the environment. `RRUSH_SHOT` gives
/// a base path (an index is inserted before the extension); `RRUSH_SHOT_T` is
/// a comma-separated list of capture times in seconds (default `2`).
fn shot_plan_from_env() -> Option<ShotPlan> {
    let base = std::env::var("RRUSH_SHOT").ok()?;
    let times = std::env::var("RRUSH_SHOT_T")
        .ok()
        .map(|s| {
            s.split(',')
                .filter_map(|x| x.trim().parse::<f32>().ok())
                .collect::<Vec<_>>()
        })
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| vec![2.0]);
    Some(ShotPlan {
        base,
        times,
        next: 0,
    })
}

// ---------------------------------------------------------------------------
// Hydrogen orbitals
// ---------------------------------------------------------------------------

/// A selection of hydrogen wave functions, in atomic units (Bohr radius
/// a₀ = 1, ħ = 1). Left un-normalised — only the shape matters here.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Orbital {
    S1,
    S2,
    P2z,
    P3z,
    D3z2,
    D3xy,
}

impl Orbital {
    fn name(self) -> &'static str {
        match self {
            Orbital::S1 => "1s",
            Orbital::S2 => "2s",
            Orbital::P2z => "2p_z",
            Orbital::P3z => "3p_z",
            Orbital::D3z2 => "3d_z2",
            Orbital::D3xy => "3d_xy",
        }
    }

    /// Principal quantum number n. Energy depends only on n.
    fn n(self) -> f32 {
        match self {
            Orbital::S1 => 1.0,
            Orbital::S2 | Orbital::P2z => 2.0,
            Orbital::P3z | Orbital::D3z2 | Orbital::D3xy => 3.0,
        }
    }

    /// Energy eigenvalue Eₙ = −1/(2n²) in Hartree.
    fn energy(self) -> f32 {
        let n = self.n();
        -0.5 / (n * n)
    }

    /// Normalisation constant so that ∫|ψ|²dV = 1 (a₀ = 1). Computed
    /// analytically from ∫r^n·e^(−ar)dr = n!/a^(n+1) and the angular
    /// integrals. This makes an equal-coefficient superposition an equal
    /// *probability* mixture, so the two states contribute symmetrically.
    fn norm(self) -> f32 {
        match self {
            Orbital::S1 => 0.564_190,
            Orbital::S2 | Orbital::P2z => 0.099_736,
            Orbital::P3z | Orbital::D3xy => 0.009_851,
            Orbital::D3z2 => 0.002_843,
        }
    }

    /// Signed, normalised amplitude ψ(x, y, z). The sign encodes the phase,
    /// which we use to colour the two lobes differently.
    fn psi(self, x: f32, y: f32, z: f32) -> f32 {
        let r = (x * x + y * y + z * z).sqrt();
        let shape = match self {
            Orbital::S1 => (-r).exp(),
            Orbital::S2 => (2.0 - r) * (-r / 2.0).exp(),
            Orbital::P2z => z * (-r / 2.0).exp(),
            Orbital::P3z => z * (6.0 - r) * (-r / 3.0).exp(),
            Orbital::D3z2 => (3.0 * z * z - r * r) * (-r / 3.0).exp(),
            Orbital::D3xy => x * y * (-r / 3.0).exp(),
        };
        self.norm() * shape
    }

    /// Half-width of the cube (in a₀) that comfortably contains the cloud.
    fn view_radius(self) -> f32 {
        match self {
            Orbital::S1 => 4.5,
            Orbital::S2 | Orbital::P2z => 13.0,
            Orbital::P3z | Orbital::D3z2 | Orbital::D3xy => 26.0,
        }
    }
}

/// One weighted term cₖ·ψₖ of a superposition.
#[derive(Clone, Copy)]
struct Term {
    orbital: Orbital,
    coeff: f32,
}

/// The thing currently on screen: either a lone eigenstate or a
/// time-evolving superposition.
///
/// NOTE: every superposition preset must combine orbitals with *distinct*
/// principal numbers n. Only then do the cross terms average to zero, so the
/// sampling envelope Σ cₖ²·ψₖ² is exact and the state actually evolves in
/// time (ΔE ≠ 0). A same-n combination would be stationary *and* would make
/// the envelope miss the cross-term support.
#[derive(Clone)]
enum SimState {
    Single(Orbital),
    Superposition {
        name: &'static str,
        terms: Vec<Term>,
    },
}

impl SimState {
    /// Build the normalised superposition for preset keys 7..0.
    fn preset(index: usize) -> SimState {
        let (name, raw): (&str, &[(Orbital, f32)]) = match index {
            0 => ("1s + 2p_z", &[(Orbital::S1, 1.0), (Orbital::P2z, 1.0)]),
            1 => ("1s + 2s", &[(Orbital::S1, 1.0), (Orbital::S2, 1.0)]),
            2 => ("2p_z + 3d_z2", &[(Orbital::P2z, 1.0), (Orbital::D3z2, 1.0)]),
            _ => (
                "1s + 2p_z + 3d_z2",
                &[(Orbital::S1, 1.0), (Orbital::P2z, 1.0), (Orbital::D3z2, 1.0)],
            ),
        };
        let norm = raw.iter().map(|(_, c)| c * c).sum::<f32>().sqrt();
        let terms = raw
            .iter()
            .map(|&(orbital, c)| Term {
                orbital,
                coeff: c / norm,
            })
            .collect();
        SimState::Superposition { name, terms }
    }

    fn name(&self) -> &str {
        match self {
            SimState::Single(o) => o.name(),
            SimState::Superposition { name, .. } => name,
        }
    }

    fn is_dynamic(&self) -> bool {
        matches!(self, SimState::Superposition { .. })
    }

    fn terms(&self) -> Vec<Term> {
        match self {
            SimState::Single(o) => vec![Term {
                orbital: *o,
                coeff: 1.0,
            }],
            SimState::Superposition { terms, .. } => terms.clone(),
        }
    }

    fn view_radius(&self) -> f32 {
        self.terms()
            .iter()
            .map(|t| t.orbital.view_radius())
            .fold(0.0, f32::max)
    }

    /// Time-averaged density Σ cₖ²·ψₖ², used to place the fixed sample points.
    fn envelope(&self, x: f32, y: f32, z: f32) -> f32 {
        self.terms()
            .iter()
            .map(|t| {
                let p = t.coeff * t.orbital.psi(x, y, z);
                p * p
            })
            .sum()
    }

    /// Instantaneous complex amplitude Ψ(r,t) = Σ cₖ·ψₖ·e^(−iEₖt),
    /// returned as (real, imag).
    fn amplitude(&self, x: f32, y: f32, z: f32, t: f32) -> (f32, f32) {
        let mut re = 0.0;
        let mut im = 0.0;
        for term in self.terms() {
            let a = term.coeff * term.orbital.psi(x, y, z);
            let phase = term.orbital.energy() * t;
            re += a * phase.cos();
            im -= a * phase.sin();
        }
        (re, im)
    }

    /// A default animation speed so the dominant Bohr frequency is watchable.
    fn suggested_speed(&self) -> f32 {
        let terms = self.terms();
        let mut max_dw = 0.0f32;
        for (i, a) in terms.iter().enumerate() {
            for b in &terms[i + 1..] {
                max_dw = max_dw.max((a.orbital.energy() - b.orbital.energy()).abs());
            }
        }
        if max_dw <= f32::EPSILON {
            4.0
        } else {
            (1.5 / max_dw).clamp(1.0, 12.0)
        }
    }
}

// ---------------------------------------------------------------------------
// Resources & components
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct CurrentState(SimState);

#[derive(Resource)]
struct SimClock {
    t: f32,
    speed: f32,
    paused: bool,
}

/// Normalisation for the instantaneous density so point scale stays in range.
#[derive(Resource)]
struct DensityRef(f32);

#[derive(Resource)]
struct CameraRig {
    angle: f32,
    distance: f32,
    height: f32,
    auto_rotate: bool,
}

/// Shared handles created once at startup and reused for every point.
#[derive(Resource)]
struct CloudAssets {
    point_mesh: Handle<Mesh>,
    positive: Handle<StandardMaterial>,
    negative: Handle<StandardMaterial>,
    hues: Vec<Handle<StandardMaterial>>,
}

/// A cloud point, remembering the fixed position it samples the field at.
#[derive(Component)]
struct CloudPoint {
    home: Vec3,
}

/// Marks points that are updated every frame (superposition mode).
#[derive(Component)]
struct Animated;

/// The hue bucket a point currently uses, so we only swap materials on change.
#[derive(Component)]
struct HueBucket(usize);

/// Roles for the pieces of dynamic on-screen text.
#[derive(Component)]
enum TextRole {
    Title,
    Subtitle,
}

/// Container whose children make up the colour legend (rebuilt on change).
#[derive(Component)]
struct LegendRoot;

/// Scripted screenshots (see `shot_plan_from_env`), handy for doc stills.
#[derive(Resource)]
struct ShotPlan {
    base: String,
    times: Vec<f32>,
    next: usize,
}

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

fn setup(
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
fn positive_color() -> Color {
    Color::srgb(0.25, 0.55, 1.0)
}

/// Colour of points where ψ is negative.
fn negative_color() -> Color {
    Color::srgb(1.0, 0.45, 0.35)
}

/// Colour of phase bucket `i` on the hue wheel.
fn hue_color(i: usize) -> Color {
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

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Keyboard controls: pick a state, steer the camera, and drive the clock.
fn handle_input(
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
fn spawn_cloud(
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
fn evolve(
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
fn update_labels(
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
fn update_legend(
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
fn drive_camera(
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

/// Save a screenshot at each scheduled time (only active when `RRUSH_SHOT` is
/// set). Files are written asynchronously by Bevy over the next frames.
fn capture_screenshots(
    mut commands: Commands,
    time: Res<Time>,
    plan: Option<ResMut<ShotPlan>>,
) {
    let Some(mut plan) = plan else {
        return;
    };
    if plan.next >= plan.times.len() {
        return;
    }
    if time.elapsed_secs() >= plan.times[plan.next] {
        let (stem, ext) = plan.base.rsplit_once('.').unwrap_or((&plan.base, "png"));
        let path = format!("{stem}_{}.{ext}", plan.next);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
        plan.next += 1;
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Map a key/argument ("1".."6" orbitals, "7".."0" presets) to a state.
fn state_from_key(key: &str) -> Option<SimState> {
    Some(match key {
        "1" => SimState::Single(Orbital::S1),
        "2" => SimState::Single(Orbital::S2),
        "3" => SimState::Single(Orbital::P2z),
        "4" => SimState::Single(Orbital::P3z),
        "5" => SimState::Single(Orbital::D3z2),
        "6" => SimState::Single(Orbital::D3xy),
        "7" => SimState::preset(0),
        "8" => SimState::preset(1),
        "9" => SimState::preset(2),
        "0" => SimState::preset(3),
        _ => return None,
    })
}

fn sample_cube(rng: &mut impl Rng, reach: f32) -> (f32, f32, f32) {
    (
        rng.random_range(-reach..reach),
        rng.random_range(-reach..reach),
        rng.random_range(-reach..reach),
    )
}

/// Map a probability density to a point scale (volume ∝ density).
fn point_scale(density: f32, reference: f32) -> Vec3 {
    let s = (density / reference).clamp(0.0, 1.0).powf(1.0 / 3.0) * 1.3;
    Vec3::splat(s)
}

/// Map a phase angle in (−π, π] to a hue-palette index.
fn hue_bucket(phase: f32) -> usize {
    let frac = (phase + std::f32::consts::PI) / std::f32::consts::TAU;
    ((frac * HUES as f32) as usize).min(HUES - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Numerically integrate the density's centre of charge ⟨z⟩ at time `t`.
    fn mean_z(state: &SimState, t: f32) -> f32 {
        let reach = state.view_radius();
        let steps = 60;
        let step = 2.0 * reach / steps as f32;
        let mut num = 0.0f64;
        let mut den = 0.0f64;
        for i in 0..steps {
            let x = -reach + (i as f32 + 0.5) * step;
            for j in 0..steps {
                let y = -reach + (j as f32 + 0.5) * step;
                for k in 0..steps {
                    let z = -reach + (k as f32 + 0.5) * step;
                    let (re, im) = state.amplitude(x, y, z, t);
                    let d = (re * re + im * im) as f64;
                    num += z as f64 * d;
                    den += d;
                }
            }
        }
        (num / den) as f32
    }

    /// The 1s + 2p_z dipole must slosh along z: ⟨z⟩ oscillates, is ~0 at a
    /// quarter period, and flips sign at the half period. This is the check
    /// that the *instantaneous* |Ψ(r,t)|² is rendered, not the time average.
    #[test]
    fn dipole_oscillates_along_z() {
        let state = SimState::preset(0); // 1s + 2p_z
        let dw = (Orbital::S1.energy() - Orbital::P2z.energy()).abs(); // 0.375
        let period = std::f32::consts::TAU / dw;

        let z0 = mean_z(&state, 0.0);
        let z_quarter = mean_z(&state, period * 0.25);
        let z_half = mean_z(&state, period * 0.5);

        // Real displacement at the extremes — near the known transition
        // dipole ⟨1s|z|2p_z⟩ = 128√2/243 ≈ 0.745 a₀.
        assert!(z0.abs() > 0.5, "expected a displaced charge cloud, got {z0}");
        // Opposite side half a period later.
        assert!(
            z0 * z_half < 0.0 && (z0 + z_half).abs() < 0.1 * z0.abs(),
            "expected ⟨z⟩ to flip sign: z0={z0}, z_half={z_half}"
        );
        // Balanced at the quarter period.
        assert!(
            z_quarter.abs() < 0.1 * z0.abs(),
            "expected ⟨z⟩≈0 at quarter period, got {z_quarter}"
        );
    }

    /// The time-averaged envelope must be centred (it is symmetric in z), so a
    /// static rendering would sit at the origin — proving the animation, not
    /// the envelope, is what produces the motion above.
    #[test]
    fn envelope_is_centred() {
        let state = SimState::preset(0);
        let reach = state.view_radius();
        let steps = 60;
        let step = 2.0 * reach / steps as f32;
        let mut num = 0.0f64;
        let mut den = 0.0f64;
        for i in 0..steps {
            let x = -reach + (i as f32 + 0.5) * step;
            for j in 0..steps {
                let y = -reach + (j as f32 + 0.5) * step;
                for k in 0..steps {
                    let z = -reach + (k as f32 + 0.5) * step;
                    let d = state.envelope(x, y, z) as f64;
                    num += z as f64 * d;
                    den += d;
                }
            }
        }
        assert!((num / den).abs() < 0.05, "envelope ⟨z⟩ should be ~0");
    }
}
