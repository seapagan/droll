use super::{DieLifecycle, TransitionReason};

#[derive(Clone, Debug)]
pub struct TransitionSample {
    pub at_seconds: f32,
    pub from: DieLifecycle,
    pub to: DieLifecycle,
    pub reason: TransitionReason,
    pub linear_speed: f32,
    pub angular_speed: f32,
    pub angular_error: f32,
    pub kinetic_energy: f32,
}

#[derive(bevy::prelude::Component, Clone, Debug, Default)]
pub struct DieMetrics {
    pub max_linear_speed: f32,
    pub max_angular_speed: f32,
    pub first_contact_seconds: Option<f32>,
    pub first_guidance_seconds: Option<f32>,
    pub guidance_entry_linear_speed: Option<f32>,
    pub guidance_entry_angular_speed: Option<f32>,
    pub guidance_entry_angular_error: Option<f32>,
    pub guidance_entry_kinetic_energy: Option<f32>,
    pub recovery_entry_angular_error: Option<f32>,
    pub recovery_entry_kinetic_energy: Option<f32>,
    pub terminal_seconds: Option<f32>,
    pub max_guidance_torque: f32,
    pub max_guidance_angular_acceleration: f32,
    pub max_guided_linear_speed: f32,
    pub max_guided_angular_speed: f32,
    pub max_recovery_linear_impulse: f32,
    pub max_recovery_angular_impulse: f32,
    pub recovery_impulses_applied: u8,
    pub transitions: Vec<TransitionSample>,
}

impl DieMetrics {
    pub fn observe(&mut self, linear_speed: f32, angular_speed: f32) {
        self.max_linear_speed = self.max_linear_speed.max(linear_speed);
        self.max_angular_speed = self.max_angular_speed.max(angular_speed);
    }

    pub fn record(&mut self, sample: TransitionSample) {
        if sample.reason == TransitionReason::FirstContact {
            self.first_contact_seconds.get_or_insert(sample.at_seconds);
        }
        if sample.to == DieLifecycle::GuidedSettling {
            self.first_guidance_seconds.get_or_insert(sample.at_seconds);
            self.guidance_entry_linear_speed
                .get_or_insert(sample.linear_speed);
            self.guidance_entry_angular_speed
                .get_or_insert(sample.angular_speed);
            self.guidance_entry_angular_error
                .get_or_insert(sample.angular_error);
            self.guidance_entry_kinetic_energy
                .get_or_insert(sample.kinetic_energy);
        }
        if sample.to == DieLifecycle::Recovery {
            self.recovery_entry_angular_error
                .get_or_insert(sample.angular_error);
            self.recovery_entry_kinetic_energy
                .get_or_insert(sample.kinetic_energy);
        }
        if matches!(sample.to, DieLifecycle::Revealed | DieLifecycle::Failed) {
            self.terminal_seconds = Some(sample.at_seconds);
        }
        self.transitions.push(sample);
    }

    pub fn observe_guided_motion(&mut self, linear_speed: f32, angular_speed: f32) {
        self.max_guided_linear_speed = self.max_guided_linear_speed.max(linear_speed);
        self.max_guided_angular_speed = self.max_guided_angular_speed.max(angular_speed);
    }

    #[must_use]
    pub fn summary(&self, lifecycle: DieLifecycle, recoveries: u8) -> MetricSummary {
        MetricSummary {
            lifecycle,
            recoveries,
            completion_seconds: self.terminal_seconds,
            max_linear_speed: self.max_linear_speed,
            max_angular_speed: self.max_angular_speed,
            max_guidance_torque: self.max_guidance_torque,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MetricSummary {
    pub lifecycle: DieLifecycle,
    pub recoveries: u8,
    pub completion_seconds: Option<f32>,
    pub max_linear_speed: f32,
    pub max_angular_speed: f32,
    pub max_guidance_torque: f32,
}
