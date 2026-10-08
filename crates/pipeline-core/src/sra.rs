use crate::{Accession, AccessionLevel, ToolKind, ToolRegistry};
use std::{
    ffi::OsString,
    fmt,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

impl CommandSpec {
    pub fn new(program: impl Into<PathBuf>, args: Vec<OsString>) -> Self {
        Self {
            program: program.into(),
            args,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefetchPlan {
    pub accession: Accession,
    pub accession_directory: PathBuf,
    pub command: CommandSpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationPlan {
    pub accession_directory: PathBuf,
    pub command: CommandSpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FasterqPlan {
    pub accession_directory: PathBuf,
    pub output_directory: PathBuf,
    pub temp_directory: PathBuf,
    pub threads: u32,
    pub command: CommandSpec,
}

#[derive(Debug, Clone)]
pub struct SraToolkitPlanner {
    tools: ToolRegistry,
}

impl SraToolkitPlanner {
    pub fn new(tools: ToolRegistry) -> Self {
        Self { tools }
    }

    pub fn tools(&self) -> &ToolRegistry {
        &self.tools
    }

    pub fn prefetch(
        &self,
        accession: &Accession,
        output_root: impl AsRef<Path>,
    ) -> Result<PrefetchPlan, SraPlanError> {
        if accession.level() != AccessionLevel::Run {
            return Err(SraPlanError::RunAccessionRequired(accession.to_string()));
        }

        let output_root = output_root.as_ref();
        if output_root.as_os_str().is_empty() {
            return Err(SraPlanError::EmptyPath("prefetch output root"));
        }

        let accession_directory = output_root.join(accession.as_str());
        let command = CommandSpec::new(
            self.tools.tool(ToolKind::Prefetch).path.clone(),
            vec![
                OsString::from(accession.as_str()),
                OsString::from("--max-size"),
                OsString::from("u"),
                OsString::from("-O"),
                output_root.as_os_str().to_owned(),
            ],
        );

        Ok(PrefetchPlan {
            accession: accession.clone(),
            accession_directory,
            command,
        })
    }

    pub fn validate(
        &self,
        accession_directory: impl AsRef<Path>,
    ) -> Result<ValidationPlan, SraPlanError> {
        let accession_directory =
            checked_path(accession_directory.as_ref(), "accession directory")?;
        let command = CommandSpec::new(
            self.tools.tool(ToolKind::VdbValidate).path.clone(),
            vec![accession_directory.as_os_str().to_owned()],
        );

        Ok(ValidationPlan {
            accession_directory,
            command,
        })
    }

    pub fn fasterq(
        &self,
        accession_directory: impl AsRef<Path>,
        output_directory: impl AsRef<Path>,
        temp_directory: impl AsRef<Path>,
        threads: u32,
    ) -> Result<FasterqPlan, SraPlanError> {
        if threads == 0 {
            return Err(SraPlanError::ZeroThreads);
        }

        let accession_directory = checked_path(accession_directory.as_ref(), "accession directory")?;
        let output_directory = checked_path(output_directory.as_ref(), "FASTQ output directory")?;
        let temp_directory = checked_path(temp_directory.as_ref(), "temporary directory")?;

        let command = CommandSpec::new(
            self.tools.tool(ToolKind::FasterqDump).path.clone(),
            vec![
                accession_directory.as_os_str().to_owned(),
                OsString::from("-O"),
                output_directory.as_os_str().to_owned(),
                OsString::from("-t"),
                temp_directory.as_os_str().to_owned(),
                OsString::from("-e"),
                OsString::from(threads.to_string()),
            ],
        );

        Ok(FasterqPlan {
            accession_directory,
            output_directory,
            temp_directory,
            threads,
            command,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SraPlanError {
    RunAccessionRequired(String),
    EmptyPath(&'static str),
    ZeroThreads,
}

impl fmt::Display for SraPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RunAccessionRequired(accession) => {
                write!(
                    f,
                    "SRA Toolkit acquisition requires a run accession after resolution: {accession}"
                )
            }
            Self::EmptyPath(label) => write!(f, "{label} must not be empty"),
            Self::ZeroThreads => f.write_str("fasterq-dump thread count must be greater than zero"),
        }
    }
}

impl std::error::Error for SraPlanError {}

fn checked_path(path: &Path, label: &'static str) -> Result<PathBuf, SraPlanError> {
    if path.as_os_str().is_empty() {
        Err(SraPlanError::EmptyPath(label))
    } else {
        Ok(path.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ToolInfo, ToolKind};

    fn registry() -> ToolRegistry {
        ToolRegistry {
            prefetch: ToolInfo {
                kind: ToolKind::Prefetch,
                path: PathBuf::from("/tools/prefetch"),
                version_output: "prefetch : 3.4.1".to_owned(),
            },
            vdb_validate: ToolInfo {
                kind: ToolKind::VdbValidate,
                path: PathBuf::from("/tools/vdb-validate"),
                version_output: "vdb-validate : 3.4.1".to_owned(),
            },
            fasterq_dump: ToolInfo {
                kind: ToolKind::FasterqDump,
                path: PathBuf::from("/tools/fasterq-dump"),
                version_output: "fasterq-dump : 3.4.1".to_owned(),
            },
        }
    }

    fn args_as_strings(spec: &CommandSpec) -> Vec<String> {
        spec.args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn prefetch_plan_is_stable_for_resume_identity() {
        let planner = SraToolkitPlanner::new(registry());
        let accession = Accession::parse("SRR4453783").expect("run accession");

        let plan = planner
            .prefetch(&accession, "/data/sra")
            .expect("prefetch plan");

        assert_eq!(
            plan.accession_directory,
            PathBuf::from("/data/sra/SRR4453783")
        );
        assert_eq!(
            args_as_strings(&plan.command),
            vec!["SRR4453783", "--max-size", "u", "-O", "/data/sra"]
        );

        let retry = planner
            .prefetch(&accession, "/data/sra")
            .expect("same retry plan");
        assert_eq!(retry, plan);
    }

    #[test]
    fn prefetch_rejects_non_run_accession() {
        let planner = SraToolkitPlanner::new(registry());
        let study = Accession::parse("SRP000001").expect("study accession");

        assert!(matches!(
            planner.prefetch(&study, "/data/sra"),
            Err(SraPlanError::RunAccessionRequired(_))
        ));
    }

    #[test]
    fn validation_uses_accession_directory() {
        let planner = SraToolkitPlanner::new(registry());
        let plan = planner
            .validate("/data/sra/SRR4453783")
            .expect("validation plan");

        assert_eq!(args_as_strings(&plan.command), vec!["/data/sra/SRR4453783"]);
    }

    #[test]
    fn fasterq_uses_container_directory_and_explicit_paths() {
        let planner = SraToolkitPlanner::new(registry());
        let plan = planner
            .fasterq(
                "/data/sra/SRR4453783",
                "/data/fastq",
                "/data/tmp/SRR4453783",
                8,
            )
            .expect("fasterq plan");

        assert_eq!(plan.accession_directory, PathBuf::from("/data/sra/SRR4453783"));
        assert_eq!(
            args_as_strings(&plan.command),
            vec![
                "/data/sra/SRR4453783",
                "-O",
                "/data/fastq",
                "-t",
                "/data/tmp/SRR4453783",
                "-e",
                "8"
            ]
        );
    }

    #[test]
    fn fasterq_rejects_zero_threads() {
        let planner = SraToolkitPlanner::new(registry());

        assert_eq!(
            planner
                .fasterq("/data/sra/SRR1", "/data/fastq", "/data/tmp", 0)
                .expect_err("zero threads must fail"),
            SraPlanError::ZeroThreads
        );
    }
}
