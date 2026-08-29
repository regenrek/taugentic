mod credentials;
mod error;
mod github;

pub use credentials::{CodeHostAccessToken, CodeHostCredentialStore, GitHttpsAuthorization};
pub use error::CodeHostError;
pub use github::{
    GitHubClient, GitHubIdentity, GitHubIssue, GitHubIssueLabel, GitHubIssuePage,
    GitHubPullRequestActivityPage, GitHubPullRequestPage, github_https_repository_url,
    github_repository_from_remote_url,
};
