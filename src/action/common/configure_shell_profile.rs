use crate::action::base::CreateOrAppendFile;
use crate::action::{Action, ActionDescription, ActionError, StatefulAction};

use std::path::Path;
use tokio::task::JoinSet;

const PROFILE_TARGETS: &[&str] = &[
    "/etc/bashrc",
    "/etc/profile.d/nix.sh",
    "/etc/zshrc",
    "/etc/bash.bashrc",
    "/etc/zsh/zshrc",
    // TODO(@hoverbear): FIsh
];
const PROFILE_NIX_FILE: &str = "/nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh";

/**
Configure any detected shell profiles to include Nix support
 */
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct ConfigureShellProfile {
    create_or_append_files: Vec<StatefulAction<CreateOrAppendFile>>,
}

impl ConfigureShellProfile {
    #[tracing::instrument(skip_all)]
    pub async fn plan() -> Result<StatefulAction<Self>, ActionError> {
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
            create_or_append_files
                .push(CreateOrAppendFile::plan(path, None, None, 0o0644, buf).await?);
        }

        Ok(Self {
            create_or_append_files,
        }
        .into())
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
    async fn execute(&mut self) -> Result<(), ActionError> {
        let Self {
            create_or_append_files,
        } = self;

        let mut set = JoinSet::new();
        let mut errors = Vec::default();

        for (idx, create_or_append_file) in create_or_append_files.iter().enumerate() {
            let mut create_or_append_file_clone = create_or_append_file.clone();
            let _abort_handle = set.spawn(async move {
                create_or_append_file_clone.try_execute().await?;
                Result::<_, ActionError>::Ok((idx, create_or_append_file_clone))
            });
        }

        while let Some(result) = set.join_next().await {
            match result {
                Ok(Ok((idx, create_or_append_file))) => {
                    create_or_append_files[idx] = create_or_append_file
                },
                Ok(Err(e)) => errors.push(Box::new(e)),
                Err(e) => return Err(e.into()),
            };
        }

        if !errors.is_empty() {
            if errors.len() == 1 {
                return Err(errors.into_iter().next().unwrap().into());
            } else {
                return Err(ActionError::Children(errors));
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
    async fn revert(&mut self) -> Result<(), ActionError> {
        let Self {
            create_or_append_files,
        } = self;

        let mut set = JoinSet::new();
        let mut errors: Vec<Box<ActionError>> = Vec::default();

        for (idx, create_or_append_file) in create_or_append_files.iter().enumerate() {
            let mut create_or_append_file_clone = create_or_append_file.clone();
            let _abort_handle = set.spawn(async move {
                create_or_append_file_clone.try_revert().await?;
                Result::<_, _>::Ok((idx, create_or_append_file_clone))
            });
        }

        while let Some(result) = set.join_next().await {
            match result {
                Ok(Ok((idx, create_or_append_file))) => {
                    create_or_append_files[idx] = create_or_append_file
                },
                Ok(Err(e)) => errors.push(Box::new(e)),
                Err(e) => return Err(e.into()),
            };
        }

        if !errors.is_empty() {
            if errors.len() == 1 {
                return Err(errors.into_iter().next().unwrap().into());
            } else {
                return Err(ActionError::Children(errors));
            }
        }

        Ok(())
    }
}
