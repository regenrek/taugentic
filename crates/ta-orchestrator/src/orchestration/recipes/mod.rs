mod builtin;
mod resolution;

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use ta_protocol::wire::{CapsuleRecipe, RecipeValidationError};

pub use resolution::{
    DelegateRecipeResolutionRequest, ResolvedDelegateRecipeRequest, resolve_delegate_recipe,
};

#[derive(Debug)]
pub struct RecipeRegistry {
    recipes: HashMap<String, CapsuleRecipe>,
}

#[derive(Debug)]
pub struct RegistryLoadOutcome {
    pub registry: RecipeRegistry,
    pub diagnostics: Vec<RecipeLoadDiagnostic>,
}

#[derive(Debug)]
pub struct RecipeLoadDiagnostic {
    pub path: PathBuf,
    pub error: RecipeRegistryError,
}

impl RecipeRegistry {
    pub fn load_builtin() -> Result<Self, RecipeRegistryError> {
        let mut registry = Self {
            recipes: HashMap::with_capacity(builtin::BUILTIN_RECIPE_SOURCES.len()),
        };

        for source in &builtin::BUILTIN_RECIPE_SOURCES {
            let recipe = parse_recipe(source.contents, PathBuf::from(source.path))?;
            registry.insert(recipe)?;
        }

        Ok(registry)
    }

    pub fn load_with_user_dir(
        user_dir: Option<&Path>,
    ) -> Result<RegistryLoadOutcome, RecipeRegistryError> {
        let mut registry = Self::load_builtin()?;
        let mut diagnostics = Vec::new();

        let Some(user_dir) = user_dir else {
            return Ok(RegistryLoadOutcome {
                registry,
                diagnostics,
            });
        };

        for path in user_recipe_paths(user_dir)? {
            let load_result = fs::read_to_string(&path)
                .map_err(|source| RecipeRegistryError::Io {
                    path: path.clone(),
                    source,
                })
                .and_then(|contents| parse_recipe(&contents, path.clone()))
                .and_then(|recipe| registry.insert(recipe));

            if let Err(error) = load_result {
                diagnostics.push(RecipeLoadDiagnostic {
                    path: path.clone(),
                    error,
                });
            }
        }

        Ok(RegistryLoadOutcome {
            registry,
            diagnostics,
        })
    }

    pub fn get(&self, id: &str) -> Option<&CapsuleRecipe> {
        self.recipes.get(id)
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.recipes.keys().map(String::as_str)
    }

    pub fn recipes(&self) -> Vec<&CapsuleRecipe> {
        let mut recipes = self.recipes.values().collect::<Vec<_>>();
        recipes.sort_by(|left, right| left.id.cmp(&right.id));
        recipes
    }

    fn insert(&mut self, recipe: CapsuleRecipe) -> Result<(), RecipeRegistryError> {
        if self.recipes.contains_key(&recipe.id) {
            return Err(RecipeRegistryError::DuplicateId(recipe.id));
        }
        self.recipes.insert(recipe.id.clone(), recipe);
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RecipeRegistryError {
    #[error("recipe id collision: {0}")]
    DuplicateId(String),
    #[error("validation failed for {id}: {error}")]
    InvalidRecipe {
        id: String,
        #[source]
        error: RecipeValidationError,
    },
    #[error("io error reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("toml parse error in {path}: {source}")]
    TomlParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
}

fn parse_recipe(contents: &str, path: PathBuf) -> Result<CapsuleRecipe, RecipeRegistryError> {
    let recipe: CapsuleRecipe =
        toml::from_str(contents).map_err(|source| RecipeRegistryError::TomlParse {
            path: path.clone(),
            source,
        })?;

    recipe
        .validate()
        .map_err(|error| RecipeRegistryError::InvalidRecipe {
            id: recipe.id.clone(),
            error,
        })?;

    Ok(recipe)
}

fn user_recipe_paths(user_dir: &Path) -> Result<Vec<PathBuf>, RecipeRegistryError> {
    let mut paths = Vec::new();
    let entries = match fs::read_dir(user_dir) {
        Ok(entries) => entries,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(paths),
        Err(source) => {
            return Err(RecipeRegistryError::Io {
                path: user_dir.to_path_buf(),
                source,
            });
        }
    };

    for entry in entries {
        let entry = entry.map_err(|source| RecipeRegistryError::Io {
            path: user_dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("toml") {
            paths.push(path);
        }
    }

    paths.sort();
    Ok(paths)
}
