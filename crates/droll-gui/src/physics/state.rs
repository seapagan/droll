/// Explicit per-die lifecycle for the directed-settling spike.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DieLifecycle {
    Spawned,
    FreeThrow,
    Bouncing,
    GuidedSettling,
    RestCandidate,
    Recovery,
    Revealed,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionReason {
    ThrowStarted,
    FirstContact,
    LowEnergyNearTray,
    TargetCandidate,
    StableTarget,
    RenewedMotion,
    CandidateInvalidated,
    GuidanceStalled,
    RecoveryImpulseComplete,
    RecoveryBudgetExhausted,
    Timeout,
}

#[derive(Clone, Copy, Debug)]
pub struct DieObservation {
    pub tray_contact: bool,
    pub position_y: f32,
    pub linear_speed: f32,
    pub angular_speed: f32,
    pub angular_error: f32,
    pub upward_face: u8,
    pub target_face: u8,
}

#[derive(bevy::prelude::Component, Clone, Debug)]
pub struct DieState {
    pub lifecycle: DieLifecycle,
    pub total_seconds: f32,
    pub state_seconds: f32,
    pub stable_seconds: f32,
    pub recovery_count: u8,
}

impl Default for DieState {
    fn default() -> Self {
        Self {
            lifecycle: DieLifecycle::Spawned,
            total_seconds: 0.0,
            state_seconds: 0.0,
            stable_seconds: 0.0,
            recovery_count: 0,
        }
    }
}

impl DieState {
    pub fn transition(&mut self, next: DieLifecycle) {
        self.lifecycle = next;
        self.state_seconds = 0.0;
        self.stable_seconds = 0.0;
        if next == DieLifecycle::Recovery {
            self.recovery_count += 1;
        }
    }
}

use super::DirectedPhysicsConfig;

/// Advances lifecycle classification from measured post-writeback state.
pub fn advance_lifecycle(
    state: &mut DieState,
    observation: DieObservation,
    delta_seconds: f32,
    config: &DirectedPhysicsConfig,
) -> Option<TransitionReason> {
    if matches!(
        state.lifecycle,
        DieLifecycle::Revealed | DieLifecycle::Failed
    ) {
        return None;
    }
    state.total_seconds += delta_seconds;
    state.state_seconds += delta_seconds;
    if state.total_seconds >= config.timeout_seconds {
        state.transition(DieLifecycle::Failed);
        return Some(TransitionReason::Timeout);
    }
    let transition = next_transition(state, observation, config);
    if state.lifecycle == DieLifecycle::RestCandidate && transition.is_none() {
        state.stable_seconds += delta_seconds;
        if state.stable_seconds >= config.stable_window_seconds {
            state.transition(DieLifecycle::Revealed);
            return Some(TransitionReason::StableTarget);
        }
    }
    transition.map(|(next, reason)| {
        state.transition(next);
        reason
    })
}

fn next_transition(
    state: &DieState,
    observation: DieObservation,
    config: &DirectedPhysicsConfig,
) -> Option<(DieLifecycle, TransitionReason)> {
    match state.lifecycle {
        DieLifecycle::Spawned => Some((DieLifecycle::FreeThrow, TransitionReason::ThrowStarted)),
        DieLifecycle::FreeThrow if observation.tray_contact => {
            Some((DieLifecycle::Bouncing, TransitionReason::FirstContact))
        }
        DieLifecycle::Bouncing if can_begin_guidance(observation, config) => Some((
            DieLifecycle::GuidedSettling,
            TransitionReason::LowEnergyNearTray,
        )),
        DieLifecycle::GuidedSettling if meaningful_disturbance(observation, config) => {
            Some((DieLifecycle::Bouncing, TransitionReason::RenewedMotion))
        }
        DieLifecycle::GuidedSettling if is_target_candidate(observation, config) => Some((
            DieLifecycle::RestCandidate,
            TransitionReason::TargetCandidate,
        )),
        DieLifecycle::GuidedSettling if state.state_seconds >= config.guidance_stall_seconds => {
            if state.recovery_count >= config.recovery_budget {
                Some((
                    DieLifecycle::Failed,
                    TransitionReason::RecoveryBudgetExhausted,
                ))
            } else {
                Some((DieLifecycle::Recovery, TransitionReason::GuidanceStalled))
            }
        }
        DieLifecycle::RestCandidate if meaningful_disturbance(observation, config) => {
            Some((DieLifecycle::Bouncing, TransitionReason::RenewedMotion))
        }
        DieLifecycle::RestCandidate if !is_target_candidate(observation, config) => Some((
            DieLifecycle::GuidedSettling,
            TransitionReason::CandidateInvalidated,
        )),
        DieLifecycle::Recovery if state.state_seconds >= config.recovery_seconds => Some((
            DieLifecycle::Bouncing,
            TransitionReason::RecoveryImpulseComplete,
        )),
        _ => None,
    }
}

fn can_begin_guidance(observation: DieObservation, config: &DirectedPhysicsConfig) -> bool {
    config.guidance_enabled
        && observation.tray_contact
        && observation.position_y <= config.near_tray_height
        && observation.linear_speed <= config.guidance_linear_speed
        && observation.angular_speed <= config.guidance_angular_speed
}

fn is_target_candidate(observation: DieObservation, config: &DirectedPhysicsConfig) -> bool {
    observation.tray_contact
        && observation.upward_face == observation.target_face
        && observation.angular_error <= config.orientation_tolerance
        && observation.linear_speed <= config.rest_linear_speed
        && observation.angular_speed <= config.rest_angular_speed
}

fn meaningful_disturbance(observation: DieObservation, config: &DirectedPhysicsConfig) -> bool {
    !observation.tray_contact
        || observation.linear_speed >= config.disturbance_linear_speed
        || observation.angular_speed >= config.disturbance_angular_speed
}
