use std::fmt;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(IdError);
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdError;

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("identifier must not be empty")
    }
}

impl std::error::Error for IdError {}

id_type!(JobId);
id_type!(RunId);
id_type!(ArtifactId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_blank_identifiers() {
        assert!(JobId::new("   ").is_err());
    }

    #[test]
    fn preserves_identifier_value() {
        let id = RunId::new("SRR000001").expect("valid id");
        assert_eq!(id.as_str(), "SRR000001");
    }
}
