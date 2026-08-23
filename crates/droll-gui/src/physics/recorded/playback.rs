use std::{error::Error, fmt, sync::Arc, time::Duration};

use bevy::prelude::*;

use super::{
    presentation::{D6PresentationMapping, D20PresentationMapping},
    types::TrajectorySample,
};

/// Recorded/interpolated world-body transform owner.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaybackRoot;

/// Fixed numbered render child carrying the proper d6 symmetry.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct NumberedVisual;

/// Immutable semantic mapping attached before the first visible frame.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct FixedD6Presentation(pub D6PresentationMapping);

/// Immutable d20 semantic mapping attached before the first visible frame.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct FixedD20Presentation(pub D20PresentationMapping);

/// Transform-only playback state; it has no physics world or collider access.
#[derive(Component, Clone, Debug)]
pub struct RecordedTrajectoryPlayback {
    pub samples: Arc<[TrajectorySample]>,
    pub fixed_step: Duration,
    pub elapsed: Duration,
    pub complete: bool,
}

impl RecordedTrajectoryPlayback {
    #[must_use]
    pub fn new(samples: Arc<[TrajectorySample]>, fixed_step: Duration) -> Self {
        Self {
            samples,
            fixed_step,
            elapsed: Duration::ZERO,
            complete: false,
        }
    }
}

/// Pure sampled body transform and audit information.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SampledPlaybackTransform {
    pub world_position: Vec3,
    pub recorded_orientation: Quat,
    pub lower_index: usize,
    pub upper_index: usize,
    pub alpha: f32,
    pub at_end: bool,
}

/// Installs only transform playback; it deliberately does not install Avian.
pub struct RecordedPlaybackPlugin;

impl Plugin for RecordedPlaybackPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, advance_recorded_playback);
    }
}

fn advance_recorded_playback(
    time: Res<Time>,
    mut roots: Query<(&mut Transform, &mut RecordedTrajectoryPlayback), With<PlaybackRoot>>,
) {
    for (mut transform, mut playback) in &mut roots {
        if playback.complete {
            continue;
        }
        playback.elapsed = playback.elapsed.saturating_add(time.delta());
        let sample = sample_recorded_transform(
            playback.samples.as_ref(),
            playback.fixed_step,
            playback.elapsed,
        )
        .expect("validated recorded trajectory");
        transform.translation = sample.world_position;
        transform.rotation = sample.recorded_orientation;
        playback.complete = sample.at_end;
    }
}

/// Samples dense recorded body transforms without extrapolation.
pub fn sample_recorded_transform(
    samples: &[TrajectorySample],
    fixed_step: Duration,
    elapsed: Duration,
) -> Result<SampledPlaybackTransform, PlaybackError> {
    validate_samples(samples, fixed_step)?;
    let last = samples.len() - 1;
    let step_seconds = fixed_step.as_secs_f64();
    let sample_position = elapsed.as_secs_f64() / step_seconds;
    if sample_position >= last as f64 {
        return sampled_endpoint(&samples[last], last, true);
    }
    let lower = sample_position.floor() as usize;
    let upper = (lower + 1).min(last);
    let alpha = (sample_position - lower as f64) as f32;
    interpolate_samples(&samples[lower], &samples[upper], lower, upper, alpha)
}

fn validate_samples(
    samples: &[TrajectorySample],
    fixed_step: Duration,
) -> Result<(), PlaybackError> {
    if samples.is_empty() {
        return Err(PlaybackError::EmptyTrajectory);
    }
    if fixed_step.is_zero() {
        return Err(PlaybackError::ZeroFixedStep);
    }
    if samples.iter().any(|sample| {
        !Vec3::from_array(sample.world_position).is_finite()
            || !Quat::from_array(sample.unit_orientation).is_finite()
    }) {
        return Err(PlaybackError::NonfiniteSample);
    }
    Ok(())
}

fn sampled_endpoint(
    sample: &TrajectorySample,
    index: usize,
    at_end: bool,
) -> Result<SampledPlaybackTransform, PlaybackError> {
    let orientation = normalized(Quat::from_array(sample.unit_orientation))?;
    Ok(SampledPlaybackTransform {
        world_position: Vec3::from_array(sample.world_position),
        recorded_orientation: orientation,
        lower_index: index,
        upper_index: index,
        alpha: 0.0,
        at_end,
    })
}

fn interpolate_samples(
    lower: &TrajectorySample,
    upper: &TrajectorySample,
    lower_index: usize,
    upper_index: usize,
    alpha: f32,
) -> Result<SampledPlaybackTransform, PlaybackError> {
    let position =
        Vec3::from_array(lower.world_position).lerp(Vec3::from_array(upper.world_position), alpha);
    let lower_rotation = normalized(Quat::from_array(lower.unit_orientation))?;
    let mut upper_rotation = normalized(Quat::from_array(upper.unit_orientation))?;
    if lower_rotation.dot(upper_rotation) < 0.0 {
        upper_rotation = -upper_rotation;
    }
    let orientation = normalized(lower_rotation.slerp(upper_rotation, alpha))?;
    Ok(SampledPlaybackTransform {
        world_position: position,
        recorded_orientation: orientation,
        lower_index,
        upper_index,
        alpha,
        at_end: false,
    })
}

fn normalized(rotation: Quat) -> Result<Quat, PlaybackError> {
    let length = rotation.length();
    if !length.is_finite() || length <= f32::EPSILON {
        return Err(PlaybackError::NonfiniteOrientation);
    }
    Ok(rotation / length)
}

/// Locks the Bevy/glam right-local composition order for the visible child.
#[must_use]
pub fn compose_visible_orientation(
    recorded_orientation: Quat,
    symmetry: Quat,
    visual_base: Quat,
) -> Quat {
    (recorded_orientation * symmetry * visual_base).normalize()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackError {
    EmptyTrajectory,
    ZeroFixedStep,
    NonfiniteSample,
    NonfiniteOrientation,
}

impl fmt::Display for PlaybackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid recorded trajectory playback: {self:?}")
    }
}

impl Error for PlaybackError {}
