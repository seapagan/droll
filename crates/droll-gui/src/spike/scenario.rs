use std::{fmt, str::FromStr};

use crate::physics::{D6LaunchFamily, LaunchNuisance, d6_passing_launch_families};

/// Explicit spike implementation mode; unknown values never fall back.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpikeMode {
    CandidateD,
    RecordedReplay,
    SymmetryLaunch,
}

impl SpikeMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CandidateD => "candidate-d",
            Self::RecordedReplay => "recorded-replay",
            Self::SymmetryLaunch => "symmetry-launch",
        }
    }
}

impl FromStr for SpikeMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "candidate-d" => Ok(Self::CandidateD),
            "recorded-replay" => Ok(Self::RecordedReplay),
            "symmetry-launch" => Ok(Self::SymmetryLaunch),
            _ => Err(format!("unknown Stage 1 mode `{value}`")),
        }
    }
}

/// Stable bounded scenario names for the Stage 1 spike.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpikeScenario {
    D6Faces,
    D20Faces,
    FourD6,
    KeepDrop,
    Recovery,
}

impl SpikeScenario {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::D6Faces => "d6-faces",
            Self::D20Faces => "d20-faces",
            Self::FourD6 => "4d6",
            Self::KeepDrop => "keep-drop",
            Self::Recovery => "recovery",
        }
    }

    pub fn selected_cases(self, requested: Option<&str>) -> Result<Vec<SpikeCase>, String> {
        let cases = self.cases()?;
        match requested {
            None => Ok(cases),
            Some(id) => cases
                .into_iter()
                .find(|case| case.id == id)
                .map(|case| vec![case])
                .ok_or_else(|| format!("unknown case `{id}` for scenario `{}`", self.as_str())),
        }
    }

    pub fn cases(self) -> Result<Vec<SpikeCase>, String> {
        match self {
            Self::D6Faces => Ok(d6_cases()),
            Self::Recovery => Ok(recovery_cases()),
            _ => Err(format!(
                "scenario `{}` is reserved for a later Stage 1 checkpoint",
                self.as_str()
            )),
        }
    }
}

impl FromStr for SpikeScenario {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "d6-faces" => Ok(Self::D6Faces),
            "d20-faces" => Ok(Self::D20Faces),
            "4d6" => Ok(Self::FourD6),
            "keep-drop" => Ok(Self::KeepDrop),
            "recovery" => Ok(Self::Recovery),
            _ => Err(format!("unknown Stage 1 scenario `{value}`")),
        }
    }
}

impl fmt::Display for SpikeScenario {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartCase {
    HighTumble,
    SideSpin,
    AwkwardLowEnergy,
    RecoveryBadOrientation,
    RecoveryEdge,
    SymmetryFamilyA,
    SymmetryFamilyB,
    SymmetryFamilyC,
    SymmetryFamilyD,
}

impl StartCase {
    #[must_use]
    pub fn symmetry_family(self) -> Option<D6LaunchFamily> {
        let families = d6_passing_launch_families();
        match self {
            Self::SymmetryFamilyA => Some(families[0]),
            Self::SymmetryFamilyB => Some(families[1]),
            Self::SymmetryFamilyC => Some(families[2]),
            Self::SymmetryFamilyD => Some(families[3]),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpikeCase {
    pub id: String,
    pub scenario: SpikeScenario,
    pub target_face: u8,
    pub start: StartCase,
    pub yaw_radians: f32,
}

fn d6_cases() -> Vec<SpikeCase> {
    (1_u8..=6)
        .flat_map(|target_face| {
            [
                (StartCase::HighTumble, "high-tumble", 0.17),
                (StartCase::SideSpin, "side-spin", 1.11),
                (StartCase::AwkwardLowEnergy, "awkward-low-energy", 2.23),
            ]
            .map(move |(start, name, yaw_radians)| SpikeCase {
                id: format!("d6-{target_face}-{name}"),
                scenario: SpikeScenario::D6Faces,
                target_face,
                start,
                yaw_radians,
            })
        })
        .collect()
}

fn recovery_cases() -> Vec<SpikeCase> {
    [
        (
            "recovery-bad-orientation",
            6,
            StartCase::RecoveryBadOrientation,
            0.7,
        ),
        ("recovery-edge", 3, StartCase::RecoveryEdge, 1.8),
    ]
    .into_iter()
    .map(|(id, target_face, start, yaw_radians)| SpikeCase {
        id: id.to_owned(),
        scenario: SpikeScenario::Recovery,
        target_face,
        start,
        yaw_radians,
    })
    .collect()
}

/// Predetermined family-major, target-minor nominal sequence for owner review.
#[must_use]
pub fn symmetry_d6_cases() -> Vec<SpikeCase> {
    [
        StartCase::SymmetryFamilyA,
        StartCase::SymmetryFamilyB,
        StartCase::SymmetryFamilyC,
        StartCase::SymmetryFamilyD,
    ]
    .into_iter()
    .flat_map(|start| {
        let family = start.symmetry_family().expect("symmetry family exists");
        (1_u8..=6).map(move |target_face| SpikeCase {
            id: format!("{}-target-{target_face}", family.family_id),
            scenario: SpikeScenario::D6Faces,
            target_face,
            start,
            yaw_radians: 0.0,
        })
    })
    .collect()
}

#[must_use]
pub const fn nominal_nuisance() -> LaunchNuisance {
    LaunchNuisance::Nominal
}
