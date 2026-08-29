mod error;
mod github;
mod sync;

pub use error::WorkSourceError;
pub use github::{GitHubIssueProvider, GitHubProviderConfig};
pub use sync::FetchOutcome;
