//! Small stateless helpers shared between `main` and the systems.

use bevy::prelude::*;
use rand::Rng;

use crate::HUES;
use crate::physics::{Orbital, SimState};

/// Map a key/argument ("1".."6" orbitals, "7".."0" presets) to a state.
pub(crate) fn state_from_key(key: &str) -> Option<SimState> {
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

/// Draw a uniform random point from the cube of half-width `reach`.
pub(crate) fn sample_cube(rng: &mut impl Rng, reach: f32) -> (f32, f32, f32) {
    (
        rng.random_range(-reach..reach),
        rng.random_range(-reach..reach),
        rng.random_range(-reach..reach),
    )
}

/// Map a probability density to a point scale (volume ∝ density).
pub(crate) fn point_scale(density: f32, reference: f32) -> Vec3 {
    let s = (density / reference).clamp(0.0, 1.0).powf(1.0 / 3.0) * 1.3;
    Vec3::splat(s)
}

/// Map a phase angle in (−π, π] to a hue-palette index.
pub(crate) fn hue_bucket(phase: f32) -> usize {
    let frac = (phase + std::f32::consts::PI) / std::f32::consts::TAU;
    ((frac * HUES as f32) as usize).min(HUES - 1)
}
