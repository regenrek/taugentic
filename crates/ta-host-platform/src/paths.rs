use std::{
    io,
    path::{Path, PathBuf},
};

pub fn canonical_realpath(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    std::fs::canonicalize(path)
}

pub fn taugentic_user_recipe_dir() -> Option<PathBuf> {
    normalized_home_dir().map(|home| home.join(".taugentic").join("recipes"))
}

pub fn taugentic_workflow_file_path() -> Option<PathBuf> {
    normalized_home_dir().map(|home| home.join(".taugentic").join("workflow.yaml"))
}

fn normalized_home_dir() -> Option<PathBuf> {
    normalize_env_path(std::env::var_os("HOME"))
        .or_else(|| normalize_env_path(std::env::var_os("USERPROFILE")))
        .map(PathBuf::from)
}

fn normalize_env_path(value: Option<std::ffi::OsString>) -> Option<String> {
    value
        .map(|value| value.to_string_lossy().trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_home_recipe_dir() {
        let path = normalize_env_path(Some(" /Users/alice ".into()))
            .map(PathBuf::from)
            .map(|home| home.join(".taugentic").join("recipes"));

        assert_eq!(path, Some(PathBuf::from("/Users/alice/.taugentic/recipes")));
    }

    #[test]
    fn normalizes_home_workflow_file_path() {
        let path = normalize_env_path(Some(" /Users/alice ".into()))
            .map(PathBuf::from)
            .map(|home| home.join(".taugentic").join("workflow.yaml"));

        assert_eq!(
            path,
            Some(PathBuf::from("/Users/alice/.taugentic/workflow.yaml"))
        );
    }

    #[test]
    fn treats_whitespace_home_as_missing() {
        assert_eq!(normalize_env_path(Some("   ".into())), None);
    }
}
