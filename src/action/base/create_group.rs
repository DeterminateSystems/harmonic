use tokio::process::Command;

use crate::execute_command;

use crate::{
    action::{Action, ActionDescription, ActionState},
    BoxableError,
};

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct CreateGroup {
    name: String,
    gid: usize,
    action_state: ActionState,
}

impl CreateGroup {
    #[tracing::instrument(skip_all)]
    pub fn plan(name: String, gid: usize) -> Self {
        Self {
            name,
            gid,
            action_state: ActionState::Uncompleted,
        }
    }
}

#[async_trait::async_trait]
#[typetag::serde(name = "create_group")]
impl Action for CreateGroup {
    fn describe_execute(&self) -> Vec<ActionDescription> {
        let Self {
            name,
            gid,
            action_state: _,
        } = &self;
        if self.action_state == ActionState::Completed {
            vec![]
        } else {
            vec![ActionDescription::new(
                format!("Create group {name} with GID {gid}"),
                vec![format!(
                    "The nix daemon requires a system user group its system users can be part of"
                )],
            )]
        }
    }

    #[tracing::instrument(skip_all, fields(
        user = self.name,
        gid = self.gid,
    ))]
    async fn execute(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Self {
            name,
            gid,
            action_state,
        } = self;
        if *action_state == ActionState::Completed {
            tracing::trace!("Already completed: Creating group");
            return Ok(());
        }
        tracing::debug!("Creating group");

        use target_lexicon::OperatingSystem;
        match target_lexicon::OperatingSystem::host() {
            OperatingSystem::MacOSX {
                major: _,
                minor: _,
                patch: _,
            }
            | OperatingSystem::Darwin => {
                if Command::new("/usr/bin/dscl")
                    .args([".", "-read", &format!("/Groups/{name}")])
                    .status()
                    .await?
                    .success()
                {
                    ()
                } else {
                    execute_command(Command::new("/usr/sbin/dseditgroup").args([
                        "-o",
                        "create",
                        "-r",
                        "Nix build group for nix-daemon",
                        "-i",
                        &format!("{gid}"),
                        name.as_str(),
                    ]))
                    .await
                    .map_err(|e| CreateGroupError::Command(e).boxed())?;
                }
            },
            _ => {
                execute_command(Command::new("groupadd").args([
                    "-g",
                    &gid.to_string(),
                    "--system",
                    &name,
                ]))
                .await
                .map_err(|e| CreateGroupError::Command(e).boxed())?;
            },
        };

        tracing::trace!("Created group");
        *action_state = ActionState::Completed;
        Ok(())
    }

    fn describe_revert(&self) -> Vec<ActionDescription> {
        let Self {
            name,
            gid: _,
            action_state: _,
        } = &self;
        if self.action_state == ActionState::Completed {
            vec![]
        } else {
            vec![ActionDescription::new(
                format!("Delete group {name}"),
                vec![format!(
                    "The nix daemon requires a system user group its system users can be part of"
                )],
            )]
        }
    }

    #[tracing::instrument(skip_all, fields(
        user = self.name,
        gid = self.gid,
    ))]
    async fn revert(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Self {
            name,
            gid: _,
            action_state,
        } = self;
        if *action_state == ActionState::Uncompleted {
            tracing::trace!("Already reverted: Deleting group");
            return Ok(());
        }
        tracing::debug!("Deleting group");

        use target_lexicon::OperatingSystem;
        match target_lexicon::OperatingSystem::host() {
            OperatingSystem::MacOSX {
                major: _,
                minor: _,
                patch: _,
            }
            | OperatingSystem::Darwin => {
                // TODO(@hoverbear): Make this actually work...
                // Right now, our test machines do not have a secure token and cannot delete users.
                tracing::warn!("Harmonic currently cannot delete groups on Mac due to https://github.com/DeterminateSystems/harmonic/issues/33. This is a no-op, installing with harmonic again will use the existing group.");
                // execute_command(Command::new("/usr/bin/dscl").args([
                //     ".",
                //     "-delete",
                //     &format!("/Groups/{name}"),
                // ]))
                // .await
                // .map_err(|e| CreateGroupError::Command(e).boxed())?;
            },
            _ => {
                execute_command(Command::new("groupdel").arg(&name))
                    .await
                    .map_err(|e| CreateGroupError::Command(e).boxed())?;
            },
        };

        tracing::trace!("Deleted group");
        *action_state = ActionState::Uncompleted;
        Ok(())
    }

    fn action_state(&self) -> ActionState {
        self.action_state
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreateGroupError {
    #[error("Failed to execute command")]
    Command(#[source] std::io::Error),
}
