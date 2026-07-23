//! Scripted screenshots, handy for generating documentation stills and GIF
//! frames. Only active when the `RRUSH_SHOT` environment variable is set.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

/// A plan of capture times, read from the environment at startup.
#[derive(Resource)]
pub(crate) struct ShotPlan {
    base: String,
    times: Vec<f32>,
    next: usize,
}

/// Read an optional screenshot plan from the environment. `RRUSH_SHOT` gives
/// a base path (an index is inserted before the extension); `RRUSH_SHOT_T` is
/// a comma-separated list of capture times in seconds (default `2`).
pub(crate) fn shot_plan_from_env() -> Option<ShotPlan> {
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

/// Save a screenshot at each scheduled time (only active when `RRUSH_SHOT` is
/// set). Files are written asynchronously by Bevy over the next frames.
pub(crate) fn capture_screenshots(
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
