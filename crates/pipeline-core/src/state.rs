use std::fmt;

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
                | (PausedAtBoundary, Converting | Cancelled)
                | (FastqReady, Compressing | Checksumming | Failed | Cancelled)
                | (Compressing, Checksumming | Failed | Cancelled)
                | (Checksumming, Complete | Failed | Cancelled)
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
    fn conversion_pause_resumes_from_boundary() {
        assert!(RunState::Converting.can_transition_to(RunState::PausedAtBoundary));
        assert!(RunState::PausedAtBoundary.can_transition_to(RunState::Converting));
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
