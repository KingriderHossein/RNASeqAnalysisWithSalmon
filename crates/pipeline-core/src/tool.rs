use std::{
    fmt,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolKind {
    Prefetch,
    VdbValidate,
    FasterqDump,
}

impl ToolKind {
    pub const SRA_REQUIRED: [Self; 3] = [
        Self::Prefetch,
        Self::VdbValidate,
        Self::FasterqDump,
    ];

    pub fn binary_name(self) -> &'static str {
        match self {
            Self::Prefetch => "prefetch",
            Self::VdbValidate => "vdb-validate",
            Self::FasterqDump => "fasterq-dump",
        }
    }
}

impl fmt::Display for ToolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.binary_name())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolInfo {
    pub kind: ToolKind,
    pub path: PathBuf,
    pub version_output: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRegistry {
    pub prefetch: ToolInfo,
    pub vdb_validate: ToolInfo,
    pub fasterq_dump: ToolInfo,
}

impl ToolRegistry {
    pub fn discover_sra_toolkit() -> Result<Self, ToolError> {
        Ok(Self {
            prefetch: discover_tool(ToolKind::Prefetch)?,
            vdb_validate: discover_tool(ToolKind::VdbValidate)?,
            fasterq_dump: discover_tool(ToolKind::FasterqDump)?,
        })
    }

    pub fn tool(&self, kind: ToolKind) -> &ToolInfo {
        match kind {
            ToolKind::Prefetch => &self.prefetch,
            ToolKind::VdbValidate => &self.vdb_validate,
            ToolKind::FasterqDump => &self.fasterq_dump,
        }
    }
}

#[derive(Debug)]
pub enum ToolError {
    Missing {
        kind: ToolKind,
        source: which::Error,
    },
    VersionSpawn {
        kind: ToolKind,
        path: PathBuf,
        source: std::io::Error,
    },
    VersionFailed {
        kind: ToolKind,
        path: PathBuf,
        exit_code: Option<i32>,
        output: String,
    },
    EmptyVersion {
        kind: ToolKind,
        path: PathBuf,
    },
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { kind, .. } => {
                write!(f, "required tool not found on PATH: {kind}")
            }
            Self::VersionSpawn { kind, path, source } => {
                write!(
                    f,
                    "cannot execute {kind} at {} to read its version: {source}",
                    path.display()
                )
            }
            Self::VersionFailed {
                kind,
                path,
                exit_code,
                output,
            } => {
                write!(
                    f,
                    "{kind} version probe failed at {} with exit code {:?}: {}",
                    path.display(),
                    exit_code,
                    output
                )
            }
            Self::EmptyVersion { kind, path } => {
                write!(
                    f,
                    "{kind} at {} returned no version information",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for ToolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Missing { source, .. } => Some(source),
            Self::VersionSpawn { source, .. } => Some(source),
            Self::VersionFailed { .. } | Self::EmptyVersion { .. } => None,
        }
    }
}

pub fn discover_tool(kind: ToolKind) -> Result<ToolInfo, ToolError> {
    let path = which::which(kind.binary_name()).map_err(|source| ToolError::Missing {
        kind,
        source,
    })?;
    let version_output = probe_version(kind, &path)?;
    Ok(ToolInfo {
        kind,
        path,
        version_output,
    })
}

pub fn probe_version(kind: ToolKind, path: &Path) -> Result<String, ToolError> {
    let output = Command::new(path)
        .arg("--version")
        .output()
        .map_err(|source| ToolError::VersionSpawn {
            kind,
            path: path.to_path_buf(),
            source,
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let combined = match (stdout.is_empty(), stderr.is_empty()) {
        (false, false) => format!("{stdout}\n{stderr}"),
        (false, true) => stdout,
        (true, false) => stderr,
        (true, true) => String::new(),
    };

    if !output.status.success() {
        return Err(ToolError::VersionFailed {
            kind,
            path: path.to_path_buf(),
            exit_code: output.status.code(),
            output: combined,
        });
    }

    if combined.is_empty() {
        return Err(ToolError::EmptyVersion {
            kind,
            path: path.to_path_buf(),
        });
    }

    Ok(combined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_names_are_stable() {
        assert_eq!(ToolKind::Prefetch.binary_name(), "prefetch");
        assert_eq!(ToolKind::VdbValidate.binary_name(), "vdb-validate");
        assert_eq!(ToolKind::FasterqDump.binary_name(), "fasterq-dump");
    }

    #[test]
    fn missing_random_tool_is_actionable() {
        let random_name = format!(
            "rnaseq-pipeline-tool-that-does-not-exist-{}",
            std::process::id()
        );
        let error = which::which(&random_name).expect_err("random tool must be absent");
        let wrapped = ToolError::Missing {
            kind: ToolKind::Prefetch,
            source: error,
        };
        assert!(wrapped.to_string().contains("required tool not found on PATH"));
    }
}
