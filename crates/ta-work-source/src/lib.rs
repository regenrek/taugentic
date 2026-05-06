mod error;
mod github;
mod model;
mod sync;

pub use error::{RateLimitBackoff, RateLimitReason, WorkSourceError};
pub use github::{
    GitHubCredentialProvider, GitHubIssueProvider, GitHubProviderConfig, GitHubToken,
    GitHubTokenMigration, HostSecretsGitHubCredentialProvider,
};
pub use model::{
    GitHubRepository, WorkItem, WorkItemKey, WorkItemStatus, WorkSource, WorkSourceConfig,
    WorkSourceKind, WorkSourceLabelFilter, WorkSourceRecipeMapping,
};
pub use sync::{FetchOutcome, SourceCursor};
