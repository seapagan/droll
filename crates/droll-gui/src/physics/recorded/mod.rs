//! Target-blind hidden physics recording for the Phase 0 feasibility spike.

mod playback;
mod presentation;
mod runner;
mod types;
mod validity;

pub use playback::{
    FixedD6Presentation, NumberedVisual, PlaybackError, PlaybackRoot, RecordedPlaybackPlugin,
    RecordedTrajectoryPlayback, SampledPlaybackTransform, compose_visible_orientation,
    sample_recorded_transform,
};
pub use presentation::{
    D6PresentationMapping, D6PresentationPhase, PresentationMapError, SemanticPresentationMap,
    map_d6_presentation, natural_record_identity,
};
pub use runner::prepare_recorded_batch;
pub use types::{
    AttemptDiagnostic, AttemptOutcome, CalibrationMetrics, ContactDiagnostics, DieKind,
    InvalidityReason, NaturalTerminalDiagnostics, PhysicalBatchRequest, PhysicalTray,
    PreparationFailure, RecordedBatch, RecordedDie, SupportClassification, TrajectorySample,
};
pub use validity::PhysicalValidityPolicy;
