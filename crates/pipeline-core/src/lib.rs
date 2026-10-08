pub mod acquisition;
pub mod conversion;
pub mod identity;
pub mod input;
pub mod persistence;
pub mod process;
pub mod sra;
pub mod state;
pub mod storage;
pub mod tool;

pub use acquisition::{
    AcquisitionDisposition, AcquisitionError, AcquisitionResult, AcquisitionStage,
    SraAcquisitionExecutor,
};
pub use conversion::{
    ConversionDisposition, ConversionError, ConversionRequest, ConversionResult,
    FasterqConversionExecutor,
};
pub use identity::{ArtifactId, IdError, JobId, RunId};
pub use input::{
    parse_batch_file, Accession, AccessionKind, AccessionLevel, BatchInput, DirectUrl, InputError,
};
pub use persistence::{
    ArtifactKind, ArtifactRecord, ArtifactValidationState, JobRecord, NewArtifact, NewJob, NewRun,
    RunRecord, StateStore, StoreError,
};
pub use state::{
    JobState, RetryTransitionError, RunState, StateParseError, TransitionError, WarningSeverity,
    WorkflowWarning,
};
pub use storage::{
    StorageError, StorageInspection, StorageInspector, StoragePlan, StorageWarning,
    StorageWarningKind,
};

pub use process::{
    CommandOutcome, CommandRunner, CommandSpec, ProcessContext, ProcessError, StopToken,
    SystemCommandRunner,
};
pub use sra::{FasterqPlan, PrefetchPlan, SraPlanError, SraToolkitPlanner, ValidationPlan};
pub use tool::{discover_tool, probe_version, ToolError, ToolInfo, ToolKind, ToolRegistry};
