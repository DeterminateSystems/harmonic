use std::path::{Path, PathBuf};

use tokio::process::Command;

use crate::action::{ActionError, StatefulAction};
use crate::execute_command;

use crate::action::{Action, ActionDescription};

/**
Unmount an APFS volume
 */
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct UnmountApfsVolume {
    disk: PathBuf,
    name: String,
}

impl UnmountApfsVolume {
    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn plan(
        disk: impl AsRef<Path>,
        name: String,
    ) -> Result<StatefulAction<Self>, ActionError> {
        let disk = disk.as_ref().to_owned();
        Ok(Self { disk, name }.into())
    }
}

#[async_trait::async_trait]
#[typetag::serde(name = "unmount_volume")]
impl Action for UnmountApfsVolume {
    fn tracing_synopsis(&self) -> String {
        format!("Unmount the `{}` APFS volume", self.name)
    }

    fn execute_description(&self) -> Vec<ActionDescription> {
        vec![ActionDescription::new(self.tracing_synopsis(), vec![])]
    }

    #[tracing::instrument(level = "debug", skip_all, fields(
        disk = %self.disk.display(),
        name = %self.name,
    ))]
    async fn execute(&mut self) -> Result<(), ActionError> {
        let Self { disk: _, name } = self;

        execute_command(
            Command::new("/usr/sbin/diskutil")
                .process_group(0)
                .args(["unmount", "force"])
                .arg(name)
                .stdin(std::process::Stdio::null()),
        )
        .await
        .map_err(|e| ActionError::Command(e))?;

        Ok(())
    }

    fn revert_description(&self) -> Vec<ActionDescription> {
        vec![ActionDescription::new(self.tracing_synopsis(), vec![])]
    }

    #[tracing::instrument(level = "debug", skip_all, fields(
        disk = %self.disk.display(),
        name = %self.name,
    ))]
    async fn revert(&mut self) -> Result<(), ActionError> {
        let Self { disk: _, name } = self;

        execute_command(
            Command::new("/usr/sbin/diskutil")
                .process_group(0)
                .args(["unmount", "force"])
                .arg(name)
                .stdin(std::process::Stdio::null()),
        )
        .await
        .map_err(|e| ActionError::Command(e))?;

        Ok(())
    }
}
