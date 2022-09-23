use std::{error::Error, fmt::Display};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{InstallPlan, plan::InstallReceipt, actions::{Revertable, Actionable, ActionDescription}};

#[derive(thiserror::Error, Debug)]
pub enum HarmonicError {
    #[error("Request error")]
    Reqwest(#[from] reqwest::Error),
    #[error("Unarchiving error")]
    Unarchive(std::io::Error),
    #[error("Getting temporary directory")]
    TempDir(std::io::Error),
    #[error("Glob pattern error")]
    GlobPatternError(#[from] glob::PatternError),
    #[error("Glob globbing error")]
    GlobGlobError(#[from] glob::GlobError),
    #[error("Symlinking from `{0}` to `{1}`")]
    Symlink(std::path::PathBuf, std::path::PathBuf, std::io::Error),
    #[error("Renaming from `{0}` to `{1}`")]
    Rename(std::path::PathBuf, std::path::PathBuf, std::io::Error),
    #[error("Unarchived Nix store did not appear to include a `nss-cacert` location")]
    NoNssCacert,
    #[error("No supported init system found")]
    InitNotSupported,
    #[error("Creating file `{0}`: {1}")]
    CreateFile(std::path::PathBuf, std::io::Error),
    #[error("Creating directory `{0}`: {1}")]
    CreateDirectory(std::path::PathBuf, std::io::Error),
    #[error("Walking directory `{0}`")]
    WalkDirectory(std::path::PathBuf, walkdir::Error),
    #[error("Setting permissions `{0}`")]
    SetPermissions(std::path::PathBuf, std::io::Error),
    #[error("Command `{0}` failed to execute")]
    CommandFailedExec(String, std::io::Error),
    // TODO(@Hoverbear): This should capture the stdout.
    #[error("Command `{0}` did not to return a success status")]
    CommandFailedStatus(String),
    #[error("Join error")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("Opening file `{0}` for writing")]
    OpenFile(std::path::PathBuf, std::io::Error),
    #[error("Opening file `{0}` for writing")]
    WriteFile(std::path::PathBuf, std::io::Error),
    #[error("Seeking file `{0}` for writing")]
    SeekFile(std::path::PathBuf, std::io::Error),
    #[error("Changing ownership of `{0}`")]
    Chown(std::path::PathBuf, nix::errno::Errno),
    #[error("Getting uid for user `{0}`")]
    UserId(String, nix::errno::Errno),
    #[error("Getting user `{0}`")]
    NoUser(String),
    #[error("Getting gid for group `{0}`")]
    GroupId(String, nix::errno::Errno),
    #[error("Getting group `{0}`")]
    NoGroup(String),
    #[error("Errors with additional failures during reverts: {}\nDuring Revert:{}", .0.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join(" & "), .1.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join(" & "))]
    FailedReverts(Vec<HarmonicError>, Vec<HarmonicError>),
    #[error("Multiple errors: {}", .0.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join(" & "))]
    Multiple(Vec<HarmonicError>),
}

#[derive(thiserror::Error, Debug)]
enum NewError {
    #[error("")]
    InstallError(InstallReceipt),
}


#[derive(thiserror::Error, Debug, Clone, Serialize, Deserialize)]
pub enum ActionState<P> where P: Actionable {
    #[serde(bound = "P::Receipt: DeserializeOwned")]
    Attempted(P::Receipt),
    #[serde(bound = "P: DeserializeOwned")]
    Planned(P),
}

fn return_option_none<E: Error + Display>() -> Option<E> {
    None
}