use std::path::{Path, PathBuf};

use tokio::process::Command;

use crate::action::{ActionError, StatefulAction};
use crate::execute_command;

use crate::action::{Action, ActionDescription};

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct CreateApfsVolume {
    disk: PathBuf,
    name: String,
    case_sensitive: bool,
}

impl CreateApfsVolume {
    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn plan(
        disk: impl AsRef<Path>,
        name: String,
        case_sensitive: bool,
    ) -> Result<StatefulAction<Self>, ActionError> {
        Ok(Self {
            disk: disk.as_ref().to_path_buf(),
            name,
            case_sensitive,
        }
        .into())
    }
}

#[async_trait::async_trait]
#[typetag::serde(name = "create_volume")]
impl Action for CreateApfsVolume {
    fn tracing_synopsis(&self) -> String {
        format!(
            "Create an APFS volume on `{}` named `{}`",
            self.disk.display(),
            self.name
        )
    }

    fn execute_description(&self) -> Vec<ActionDescription> {
        vec![ActionDescription::new(self.tracing_synopsis(), vec![])]
    }

    #[tracing::instrument(level = "debug", skip_all, fields(
        disk = %self.disk.display(),
        name = %self.name,
        case_sensitive = %self.case_sensitive,
    ))]
    async fn execute(&mut self) -> Result<(), ActionError> {
        let Self {
            disk,
            name,
            case_sensitive,
        } = self;

        execute_command(
            Command::new("/usr/sbin/diskutil")
                .process_group(0)
                .args([
                    "apfs",
                    "addVolume",
                    &format!("{}", disk.display()),
                    if !*case_sensitive {
                        "APFS"
                    } else {
                        "Case-sensitive APFS"
                    },
                    name,
                    "-nomount",
                ])
                .stdin(std::process::Stdio::null()),
        )
        .await
        .map_err(|e| ActionError::Command(e))?;

        Ok(())
    }

    fn revert_description(&self) -> Vec<ActionDescription> {
        vec![ActionDescription::new(
            format!(
                "Remove the volume on `{}` named `{}`",
                self.disk.display(),
                self.name
            ),
            vec![],
        )]
    }

    #[tracing::instrument(level = "debug", skip_all, fields(
        disk = %self.disk.display(),
        name = %self.name,
        case_sensitive = %self.case_sensitive,
    ))]
    async fn revert(&mut self) -> Result<(), ActionError> {
        let Self {
            disk: _,
            name,
            case_sensitive: _,
        } = self;

        execute_command(
            Command::new("/usr/sbin/diskutil")
                .process_group(0)
                .args(["apfs", "deleteVolume", name])
                .stdin(std::process::Stdio::null()),
        )
        .await
        .map_err(|e| ActionError::Command(e))?;

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreateVolumeError {
    #[error("Failed to execute command")]
    Command(#[source] std::io::Error),
}
