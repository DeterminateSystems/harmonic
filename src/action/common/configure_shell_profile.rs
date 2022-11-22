use crate::action::base::{CreateOrAppendFile, CreateOrAppendFileError};
use crate::action::{Action, ActionDescription, ActionImplementation, ActionState};
use crate::BoxableError;

use std::path::Path;
use tokio::task::{JoinError, JoinSet};

const PROFILE_TARGETS: &[&str] = &[
    "/etc/bashrc",
    "/etc/profile.d/nix.sh",
    "/etc/zshrc",
    "/etc/bash.bashrc",
    "/etc/zsh/zshrc",
    // TODO(@hoverbear): FIsh
];
const PROFILE_NIX_FILE: &str = "/nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh";

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct ConfigureShellProfile {
    create_or_append_files: Vec<CreateOrAppendFile>,
    action_state: ActionState,
}

impl ConfigureShellProfile {
    #[tracing::instrument(skip_all)]
    pub async fn plan() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut create_or_append_files = Vec::default();
        for profile_target in PROFILE_TARGETS {
            let path = Path::new(profile_target);
            if !path.exists() {
                tracing::trace!("Did not plan to edit `{profile_target}` as it does not exist.");
                continue;
            }
            let buf = format!(
                "\n\
                # Nix\n\
                if [ -e '{PROFILE_NIX_FILE}' ]; then\n\
                . '{PROFILE_NIX_FILE}'\n\
                fi\n\
                # End Nix\n
            \n",
            );
            create_or_append_files.push(
                CreateOrAppendFile::plan(path, None, None, 0o0644, buf)
                    .await
                    .map_err(|e| e.boxed())?,
            );
        }

        Ok(Self {
            create_or_append_files,
            action_state: ActionState::Uncompleted,
        })
    }
}

#[async_trait::async_trait]
#[typetag::serde(name = "configure_shell_profile")]
impl Action for ConfigureShellProfile {
    fn tracing_synopsis(&self) -> String {
        "Configure the shell profiles".to_string()
    }

    fn execute_description(&self) -> Vec<ActionDescription> {
        vec![ActionDescription::new(
            self.tracing_synopsis(),
            vec!["Update shell profiles to import Nix".to_string()],
        )]
    }

    #[tracing::instrument(skip_all)]
    async fn execute(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Self {
            create_or_append_files,
            action_state: _,
        } = self;

        let mut set = JoinSet::new();
        let mut errors = Vec::default();

        for (idx, create_or_append_file) in create_or_append_files.iter().enumerate() {
            let mut create_or_append_file_clone = create_or_append_file.clone();
            let _abort_handle = set.spawn(async move {
                create_or_append_file_clone.try_execute().await?;
                Result::<_, Box<dyn std::error::Error + Send + Sync>>::Ok((
                    idx,
                    create_or_append_file_clone,
                ))
            });
        }

        while let Some(result) = set.join_next().await {
            match result {
                Ok(Ok((idx, create_or_append_file))) => {
                    create_or_append_files[idx] = create_or_append_file
                },
                Ok(Err(e)) => errors.push(e),
                Err(e) => return Err(e.boxed()),
            };
        }

        if !errors.is_empty() {
            if errors.len() == 1 {
                return Err(errors.into_iter().next().unwrap().into());
            } else {
                return Err(ConfigureShellProfileError::MultipleCreateOrAppendFile(errors).boxed());
            }
        }

        Ok(())
    }

    fn revert_description(&self) -> Vec<ActionDescription> {
        vec![ActionDescription::new(
            "Unconfigure the shell profiles".to_string(),
            vec!["Update shell profiles to no longer import Nix".to_string()],
        )]
    }

    #[tracing::instrument(skip_all)]
    async fn revert(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Self {
            create_or_append_files,
            action_state: _,
        } = self;

        let mut set = JoinSet::new();
        let mut errors = Vec::default();

        for (idx, create_or_append_file) in create_or_append_files.iter().enumerate() {
            let mut create_or_append_file_clone = create_or_append_file.clone();
            let _abort_handle = set.spawn(async move {
                create_or_append_file_clone.revert().await?;
                Result::<_, Box<dyn std::error::Error + Send + Sync>>::Ok((
                    idx,
                    create_or_append_file_clone,
                ))
            });
        }

        while let Some(result) = set.join_next().await {
            match result {
                Ok(Ok((idx, create_or_append_file))) => {
                    create_or_append_files[idx] = create_or_append_file
                },
                Ok(Err(e)) => errors.push(e),
                Err(e) => return Err(e.boxed()),
            };
        }

        if !errors.is_empty() {
            if errors.len() == 1 {
                return Err(errors.into_iter().next().unwrap().into());
            } else {
                return Err(ConfigureShellProfileError::MultipleCreateOrAppendFile(errors).boxed());
            }
        }

        Ok(())
    }

    fn action_state(&self) -> ActionState {
        self.action_state
    }

    fn set_action_state(&mut self, action_state: ActionState) {
        self.action_state = action_state;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigureShellProfileError {
    #[error("Creating or appending to file")]
    CreateOrAppendFile(
        #[from]
        #[source]
        CreateOrAppendFileError,
    ),
    #[error("Multiple errors: {}", .0.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join(" & "))]
    MultipleCreateOrAppendFile(Vec<Box<dyn std::error::Error + Send + Sync>>),
    #[error("Joining spawned async task")]
    Join(
        #[source]
        #[from]
        JoinError,
    ),
}
