use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DirectoryRoot(PathBuf);

impl DirectoryRoot {
    pub fn parse_user_input(input: &str) -> Result<Self, RootPathError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(RootPathError::Empty);
        }

        let raw = PathBuf::from(trimmed);
        let absolute = if raw.is_absolute() {
            raw
        } else {
            let cwd = std::env::current_dir()
                .map_err(|_| RootPathError::CannotResolveCurrentDirectory)?;
            cwd.join(raw)
        };

        Self::from_path(absolute)
    }

    pub fn from_path(path: PathBuf) -> Result<Self, RootPathError> {
        if !path.exists() {
            return Err(RootPathError::NotFound(path));
        }
        if !path.is_dir() {
            return Err(RootPathError::NotDirectory(path));
        }

        let normalized = path.canonicalize().unwrap_or(path);
        Ok(Self(normalized))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn to_path_buf(&self) -> PathBuf {
        self.0.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectRoot(DirectoryRoot);

impl ProjectRoot {
    pub fn parse_user_input(input: &str) -> Result<Self, RootPathError> {
        DirectoryRoot::parse_user_input(input).map(Self)
    }

    pub fn from_stored(path: PathBuf) -> Option<Self> {
        DirectoryRoot::from_path(path).ok().map(Self)
    }

    pub fn as_path(&self) -> &Path {
        self.0.as_path()
    }

    pub fn to_path_buf(&self) -> PathBuf {
        self.0.to_path_buf()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GlobalEnvRoot(DirectoryRoot);

impl GlobalEnvRoot {
    pub fn parse_user_input(input: &str) -> Result<Self, RootPathError> {
        DirectoryRoot::parse_user_input(input).map(Self)
    }

    pub fn from_stored(path: PathBuf) -> Option<Self> {
        DirectoryRoot::from_path(path).ok().map(Self)
    }

    pub fn as_path(&self) -> &Path {
        self.0.as_path()
    }

    pub fn to_path_buf(&self) -> PathBuf {
        self.0.to_path_buf()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveFilePath(PathBuf);

impl SaveFilePath {
    pub fn parse_user_input(
        input: &str,
        default_project_root: Option<&ProjectRoot>,
    ) -> Result<Self, SavePathError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(SavePathError::Empty);
        }

        let raw = PathBuf::from(trimmed);
        let mut absolute = if raw.is_absolute() {
            raw
        } else {
            let Some(root) = default_project_root else {
                return Err(SavePathError::MissingProjectRootForRelativePath);
            };
            root.as_path().join(raw)
        };

        if absolute.extension().is_none() {
            absolute.set_extension("http");
        }

        if absolute
            .extension()
            .and_then(|ext| ext.to_str())
            .is_none_or(|ext| !ext.eq_ignore_ascii_case("http"))
        {
            return Err(SavePathError::NotHttpFile(absolute));
        }

        Ok(Self(absolute))
    }

    pub fn to_path_buf(&self) -> PathBuf {
        self.0.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootPathError {
    Empty,
    CannotResolveCurrentDirectory,
    NotFound(PathBuf),
    NotDirectory(PathBuf),
}

impl Display for RootPathError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("Path cannot be empty"),
            Self::CannotResolveCurrentDirectory => f.write_str("Cannot resolve current directory"),
            Self::NotFound(path) => write!(f, "Path not found: {}", path.display()),
            Self::NotDirectory(path) => write!(f, "Path is not a directory: {}", path.display()),
        }
    }
}

impl std::error::Error for RootPathError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavePathError {
    Empty,
    MissingProjectRootForRelativePath,
    NotHttpFile(PathBuf),
}

impl Display for SavePathError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("Save path cannot be empty"),
            Self::MissingProjectRootForRelativePath => {
                f.write_str("Relative save paths require at least one project root")
            }
            Self::NotHttpFile(path) => {
                write!(f, "Save path must target a .http file: {}", path.display())
            }
        }
    }
}

impl std::error::Error for SavePathError {}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn save_file_path_relative_needs_project_root() {
        let err = SaveFilePath::parse_user_input("new-request", None).unwrap_err();
        assert_eq!(err, SavePathError::MissingProjectRootForRelativePath);
    }

    #[test]
    fn save_file_path_adds_http_extension() {
        let root = ProjectRoot::from_stored(std::env::temp_dir()).expect("temp root");
        let parsed = SaveFilePath::parse_user_input("abc", Some(&root)).expect("save path");
        assert_eq!(
            parsed.to_path_buf().file_name().and_then(|v| v.to_str()),
            Some("abc.http")
        );
    }

    #[test]
    fn project_root_rejects_file() {
        let dir = tempdir().expect("temp dir");
        let file_path = dir.path().join("file.txt");
        std::fs::write(&file_path, "x").expect("write file");

        assert!(ProjectRoot::from_stored(file_path).is_none());
    }
}
