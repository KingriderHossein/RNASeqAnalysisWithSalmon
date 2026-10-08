use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RunState {
    Queued,
    Resolving,
    Ready,
    Downloading,
    Paused,
    WaitingForNetwork,
    Downloaded,
    Validating,
    SraValid,
    Converting,
    PausedAtBoundary,
    FastqReady,
    Compressing,
    Checksumming,
    Complete,
    Failed,
    Cancelled,
}

impl RunState {
    pub const ALL: [Self; 17] = [
        Self::Queued,
        Self::Resolving,
        Self::Ready,
        Self::Downloading,
        Self::Paused,
        Self::WaitingForNetwork,
        Self::Downloaded,
        Self::Validating,
        Self::SraValid,
        Self::Converting,
        Self::PausedAtBoundary,
        Self::FastqReady,
        Self::Compressing,
        Self::Checksumming,
        Self::Complete,
        Self::Failed,
        Self::Cancelled,
    ];

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Complete | Self::Cancelled)
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        use RunState::*;

        matches!(
            (self, next),
            (Queued, Resolving | Cancelled)
                | (Resolving, Ready | Failed | Cancelled)
                | (Ready, Downloading | Cancelled)
                | (
                    Downloading,
                    Paused | WaitingForNetwork | Downloaded | Failed | Cancelled
                )
                | (Paused, Downloading | Cancelled)
                | (WaitingForNetwork, Downloading | Failed | Cancelled)
                | (Downloaded, Validating | Failed | Cancelled)
                | (Validating, SraValid | Failed | Cancelled)
                | (SraValid, Converting | Checksumming | Cancelled)
                | (
                    Converting,
                    PausedAtBoundary | FastqReady | Failed | Cancelled
                )
                | (
                    PausedAtBoundary,
                    Converting | Compressing | Checksumming | Cancelled
                )
                | (FastqReady, Compressing | Checksumming | Failed | Cancelled)
                | (
                    Compressing,
                    PausedAtBoundary | Checksumming | Failed | Cancelled
                )
                | (
                    Checksumming,
                    PausedAtBoundary | Complete | Failed | Cancelled
                )
                | (Failed, Cancelled)
        )
    }

    pub fn transition_to(self, next: Self) -> Result<Self, TransitionError> {
        if self.can_transition_to(next) {
            Ok(next)
        } else {
            Err(TransitionError {
                from: self,
                to: next,
            })
        }
    }

    pub fn is_retry_target(self) -> bool {
        matches!(
            self,
            Self::Resolving
                | Self::Downloading
                | Self::Validating
                | Self::Converting
                | Self::Compressing
                | Self::Checksumming
        )
    }

    pub fn can_retry_to(self, next: Self) -> bool {
        self == Self::Failed && next.is_retry_target()
    }

    pub fn retry_to(self, next: Self) -> Result<Self, RetryTransitionError> {
        if self.can_retry_to(next) {
            Ok(next)
        } else {
            Err(RetryTransitionError {
                from: self,
                to: next,
            })
        }
    }
}

impl fmt::Display for RunState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Queued => "QUEUED",
            Self::Resolving => "RESOLVING",
            Self::Ready => "READY",
            Self::Downloading => "DOWNLOADING",
            Self::Paused => "PAUSED",
            Self::WaitingForNetwork => "WAITING_FOR_NETWORK",
            Self::Downloaded => "DOWNLOADED",
            Self::Validating => "VALIDATING",
            Self::SraValid => "SRA_VALID",
            Self::Converting => "CONVERTING",
            Self::PausedAtBoundary => "PAUSED_AT_BOUNDARY",
            Self::FastqReady => "FASTQ_READY",
            Self::Compressing => "COMPRESSING",
            Self::Checksumming => "CHECKSUMMING",
            Self::Complete => "COMPLETE",
            Self::Failed => "FAILED",
            Self::Cancelled => "CANCELLED",
        };
        f.write_str(name)
    }
}

impl FromStr for RunState {
    type Err = StateParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "QUEUED" => Ok(Self::Queued),
            "RESOLVING" => Ok(Self::Resolving),
            "READY" => Ok(Self::Ready),
            "DOWNLOADING" => Ok(Self::Downloading),
            "PAUSED" => Ok(Self::Paused),
            "WAITING_FOR_NETWORK" => Ok(Self::WaitingForNetwork),
            "DOWNLOADED" => Ok(Self::Downloaded),
            "VALIDATING" => Ok(Self::Validating),
            "SRA_VALID" => Ok(Self::SraValid),
            "CONVERTING" => Ok(Self::Converting),
            "PAUSED_AT_BOUNDARY" => Ok(Self::PausedAtBoundary),
            "FASTQ_READY" => Ok(Self::FastqReady),
            "COMPRESSING" => Ok(Self::Compressing),
            "CHECKSUMMING" => Ok(Self::Checksumming),
            "COMPLETE" => Ok(Self::Complete),
            "FAILED" => Ok(Self::Failed),
            "CANCELLED" => Ok(Self::Cancelled),
            other => Err(StateParseError::new("run", other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobState {
    Queued,
    Active,
    Paused,
    WaitingForNetwork,
    Complete,
    Failed,
    Cancelled,
}

impl JobState {
    pub const ALL: [Self; 7] = [
        Self::Queued,
        Self::Active,
        Self::Paused,
        Self::WaitingForNetwork,
        Self::Complete,
        Self::Failed,
        Self::Cancelled,
    ];

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Complete | Self::Cancelled)
    }
}

impl fmt::Display for JobState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Queued => "QUEUED",
            Self::Active => "ACTIVE",
            Self::Paused => "PAUSED",
            Self::WaitingForNetwork => "WAITING_FOR_NETWORK",
            Self::Complete => "COMPLETE",
            Self::Failed => "FAILED",
            Self::Cancelled => "CANCELLED",
        };
        f.write_str(name)
    }
}

impl FromStr for JobState {
    type Err = StateParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "QUEUED" => Ok(Self::Queued),
            "ACTIVE" => Ok(Self::Active),
            "PAUSED" => Ok(Self::Paused),
            "WAITING_FOR_NETWORK" => Ok(Self::WaitingForNetwork),
            "COMPLETE" => Ok(Self::Complete),
            "FAILED" => Ok(Self::Failed),
            "CANCELLED" => Ok(Self::Cancelled),
            other => Err(StateParseError::new("job", other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionError {
    pub from: RunState,
    pub to: RunState,
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid workflow transition: {} -> {}",
            self.from, self.to
        )
    }
}

impl std::error::Error for TransitionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryTransitionError {
    pub from: RunState,
    pub to: RunState,
}

impl fmt::Display for RetryTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid retry transition: {} -> {}", self.from, self.to)
    }
}

impl std::error::Error for RetryTransitionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateParseError {
    kind: &'static str,
    value: String,
}

impl StateParseError {
    fn new(kind: &'static str, value: impl Into<String>) -> Self {
        Self {
            kind,
            value: value.into(),
        }
    }
}

impl fmt::Display for StateParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown {} state: {}", self.kind, self.value)
    }
}

impl std::error::Error for StateParseError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningSeverity {
    Info,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowWarning {
    pub code: String,
    pub message: String,
    pub severity: WarningSeverity,
}

impl WorkflowWarning {
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        severity: WarningSeverity,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            severity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_can_wait_for_network_and_resume() {
        assert_eq!(
            RunState::Downloading.transition_to(RunState::WaitingForNetwork),
            Ok(RunState::WaitingForNetwork)
        );
        assert_eq!(
            RunState::WaitingForNetwork.transition_to(RunState::Downloading),
            Ok(RunState::Downloading)
        );
    }

    #[test]
    fn safe_boundary_pause_can_resume_stage_specific_work() {
        assert!(RunState::Converting.can_transition_to(RunState::PausedAtBoundary));
        assert!(RunState::Compressing.can_transition_to(RunState::PausedAtBoundary));
        assert!(RunState::Checksumming.can_transition_to(RunState::PausedAtBoundary));
        assert!(RunState::PausedAtBoundary.can_transition_to(RunState::Converting));
        assert!(RunState::PausedAtBoundary.can_transition_to(RunState::Compressing));
        assert!(RunState::PausedAtBoundary.can_transition_to(RunState::Checksumming));
    }

    #[test]
    fn supports_download_only_path_after_validation() {
        assert!(RunState::SraValid.can_transition_to(RunState::Checksumming));
    }

    #[test]
    fn supports_fastq_without_compression() {
        assert!(RunState::FastqReady.can_transition_to(RunState::Checksumming));
    }

    #[test]
    fn cannot_skip_required_download_stages() {
        let err = RunState::Ready
            .transition_to(RunState::Complete)
            .expect_err("READY must not jump to COMPLETE");
        assert_eq!(err.from, RunState::Ready);
        assert_eq!(err.to, RunState::Complete);
    }

    #[test]
    fn complete_and_cancelled_are_terminal() {
        for state in RunState::ALL {
            assert!(!RunState::Complete.can_transition_to(state));
            assert!(!RunState::Cancelled.can_transition_to(state));
        }
        assert!(RunState::Complete.is_terminal());
        assert!(RunState::Cancelled.is_terminal());
    }

    #[test]
    fn failed_requires_explicit_retry_transition() {
        assert!(!RunState::Failed.can_transition_to(RunState::Downloading));
        assert_eq!(
            RunState::Failed.retry_to(RunState::Downloading),
            Ok(RunState::Downloading)
        );
        assert!(RunState::Failed.retry_to(RunState::Complete).is_err());
        assert!(RunState::Failed.retry_to(RunState::Cancelled).is_err());
        assert!(RunState::Ready.retry_to(RunState::Downloading).is_err());
    }

    #[test]
    fn state_names_round_trip() {
        for state in RunState::ALL {
            assert_eq!(state.to_string().parse::<RunState>(), Ok(state));
        }
        for state in JobState::ALL {
            assert_eq!(state.to_string().parse::<JobState>(), Ok(state));
        }
    }

    #[test]
    fn warnings_are_not_workflow_states() {
        let warning = WorkflowWarning::new(
            "LOW_RECOMMENDED_SPACE",
            "Available space is below the recommended workspace.",
            WarningSeverity::Warning,
        );
        assert_eq!(warning.severity, WarningSeverity::Warning);
        assert_eq!(warning.code, "LOW_RECOMMENDED_SPACE");
    }
}
