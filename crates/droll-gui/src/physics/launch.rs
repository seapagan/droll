use bevy::prelude::{EulerRot, Quat, Vec3};

const ORIENTATION_NUISANCE_AXIS: Vec3 = Vec3::new(1.0, 1.0, 0.0);

/// A complete target-independent physical launch state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct D6LaunchState {
    pub position: Vec3,
    pub orientation: Quat,
    pub linear_velocity: Vec3,
    pub angular_velocity: Vec3,
}

/// One of the four qualitatively distinct energetic search regions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchKind {
    HighTumble,
    SideSpin,
    OverheadTumble,
    DiagonalSpin,
}

impl LaunchKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HighTumble => "high-tumble",
            Self::SideSpin => "side-spin",
            Self::OverheadTumble => "overhead-tumble",
            Self::DiagonalSpin => "diagonal-spin",
        }
    }
}

/// One of the at-most-64 development search candidates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct D6LaunchCandidate {
    pub search_id: u8,
    pub kind: LaunchKind,
    pub state: D6LaunchState,
}

/// One robustness-screened base family selected without reference to a target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct D6LaunchFamily {
    pub family_id: &'static str,
    pub candidate: D6LaunchCandidate,
    pub natural_face: u8,
}

impl D6LaunchCandidate {
    #[must_use]
    pub fn id(self) -> String {
        format!("search-{:02}-{}", self.search_id, self.kind.as_str())
    }
}

/// The nominal state and eight preregistered target-independent variants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchNuisance {
    Nominal,
    PositionXPlus,
    PositionXMinus,
    OrientationPlus,
    OrientationMinus,
    LinearPlus,
    LinearMinus,
    AngularPlus,
    AngularMinus,
}

impl LaunchNuisance {
    pub const ALL: [Self; 9] = [
        Self::Nominal,
        Self::PositionXPlus,
        Self::PositionXMinus,
        Self::OrientationPlus,
        Self::OrientationMinus,
        Self::LinearPlus,
        Self::LinearMinus,
        Self::AngularPlus,
        Self::AngularMinus,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nominal => "nominal",
            Self::PositionXPlus => "position-x-plus-0.01m",
            Self::PositionXMinus => "position-x-minus-0.01m",
            Self::OrientationPlus => "orientation-world-axis-plus-0.5deg",
            Self::OrientationMinus => "orientation-world-axis-minus-0.5deg",
            Self::LinearPlus => "linear-times-1.01",
            Self::LinearMinus => "linear-times-0.99",
            Self::AngularPlus => "angular-times-1.01",
            Self::AngularMinus => "angular-times-0.99",
        }
    }

    /// Applies the nuisance before any body-local target symmetry.
    #[must_use]
    pub fn apply(self, mut state: D6LaunchState) -> D6LaunchState {
        match self {
            Self::Nominal => {}
            Self::PositionXPlus => state.position.x += 0.01,
            Self::PositionXMinus => state.position.x -= 0.01,
            Self::OrientationPlus => {
                state.orientation = orientation_nuisance(0.5) * state.orientation;
            }
            Self::OrientationMinus => {
                state.orientation = orientation_nuisance(-0.5) * state.orientation;
            }
            Self::LinearPlus => state.linear_velocity *= 1.01,
            Self::LinearMinus => state.linear_velocity *= 0.99,
            Self::AngularPlus => state.angular_velocity *= 1.01,
            Self::AngularMinus => state.angular_velocity *= 0.99,
        }
        state
    }
}

/// Returns the complete bounded 64-combination target-independent search grid.
#[must_use]
pub fn d6_launch_search_candidates() -> Vec<D6LaunchCandidate> {
    let mut candidates = Vec::with_capacity(64);
    for kind in [
        LaunchKind::HighTumble,
        LaunchKind::SideSpin,
        LaunchKind::OverheadTumble,
        LaunchKind::DiagonalSpin,
    ] {
        let base = prototype(kind);
        for height_delta in [-0.10, 0.10] {
            for yaw_delta in [0.0, 0.35] {
                for linear_scale in [0.95, 1.05] {
                    for angular_scale in [0.95, 1.05] {
                        let mut state = base;
                        state.position.y += height_delta;
                        state.orientation = Quat::from_rotation_y(yaw_delta) * state.orientation;
                        state.linear_velocity *= linear_scale;
                        state.angular_velocity *= angular_scale;
                        candidates.push(D6LaunchCandidate {
                            search_id: u8::try_from(candidates.len())
                                .expect("the search cap is 64"),
                            kind,
                            state,
                        });
                    }
                }
            }
        }
    }
    candidates
}

/// Returns the four families accepted by the bounded Linux development search.
#[must_use]
pub fn d6_passing_launch_families() -> [D6LaunchFamily; 4] {
    let candidates = d6_launch_search_candidates();
    [
        family("h1-family-a", candidates[1], 6),
        family("h1-family-b", candidates[16], 3),
        family("h1-family-c", candidates[32], 1),
        family("h1-family-d", candidates[48], 3),
    ]
}

const fn family(
    family_id: &'static str,
    candidate: D6LaunchCandidate,
    natural_face: u8,
) -> D6LaunchFamily {
    D6LaunchFamily {
        family_id,
        candidate,
        natural_face,
    }
}

fn orientation_nuisance(degrees: f32) -> Quat {
    Quat::from_axis_angle(ORIENTATION_NUISANCE_AXIS.normalize(), degrees.to_radians())
}

fn prototype(kind: LaunchKind) -> D6LaunchState {
    let (position, euler, linear_velocity, angular_velocity) = match kind {
        LaunchKind::HighTumble => (
            Vec3::new(-1.4, 3.4, -0.8),
            Vec3::new(0.6, 0.2, 1.1),
            Vec3::new(2.4, 0.4, 1.1),
            Vec3::new(8.0, 4.0, 6.0),
        ),
        LaunchKind::SideSpin => (
            Vec3::new(1.6, 2.6, -0.4),
            Vec3::new(1.3, 0.7, 0.2),
            Vec3::new(-2.0, 0.2, 1.5),
            Vec3::new(2.0, 9.0, 3.0),
        ),
        LaunchKind::OverheadTumble => (
            Vec3::new(-0.6, 3.8, 0.8),
            Vec3::new(1.0, -0.5, 0.4),
            Vec3::new(1.3, -0.1, -2.0),
            Vec3::new(5.0, 7.0, 8.0),
        ),
        LaunchKind::DiagonalSpin => (
            Vec3::new(1.2, 3.1, 1.0),
            Vec3::new(0.3, 1.1, 0.8),
            Vec3::new(-2.2, 0.6, -1.4),
            Vec3::new(7.0, -5.0, 4.0),
        ),
    };
    D6LaunchState {
        position,
        orientation: Quat::from_euler(EulerRot::XYZ, euler.x, euler.y, euler.z),
        linear_velocity,
        angular_velocity,
    }
}
