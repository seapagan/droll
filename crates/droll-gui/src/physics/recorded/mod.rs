//! Target-blind hidden physics recording for the Phase 0 feasibility spike.

mod runner;
mod types;
mod validity;

pub use runner::prepare_recorded_batch;
pub use types::{
    AttemptDiagnostic, AttemptOutcome, CalibrationMetrics, ContactDiagnostics, DieKind,
    InvalidityReason, NaturalTerminalDiagnostics, PhysicalBatchRequest, PhysicalTray,
    PreparationFailure, RecordedBatch, RecordedDie, SupportClassification, TrajectorySample,
};
pub use validity::PhysicalValidityPolicy;
