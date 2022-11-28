use crate::action::base::{CreateDirectory, CreateDirectoryError, CreateFile, CreateFileError};
use crate::action::{Action, ActionDescription, StatefulAction};

const NIX_CONF_FOLDER: &str = "/etc/nix";
const NIX_CONF: &str = "/etc/nix/nix.conf";

/**
Place the `/etc/nix.conf` file
 */
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct PlaceNixConfiguration {
    create_directory: StatefulAction<CreateDirectory>,
    create_file: StatefulAction<CreateFile>,
}

impl PlaceNixConfiguration {
    #[tracing::instrument(skip_all)]
    pub async fn plan(
        nix_build_group_name: String,
        extra_conf: Vec<String>,
        force: bool,
    ) -> Result<StatefulAction<Self>, Box<dyn std::error::Error + Send + Sync>> {
        let buf = format!(
            "\
            {extra_conf}\n\
            \n\
            build-users-group = {nix_build_group_name}\n\
            \n\
            experimental-features = nix-command flakes\n\
            \n\
            auto-optimise-store = true\n\
        ",
            extra_conf = extra_conf.join("\n"),
        );
        let create_directory =
            CreateDirectory::plan(NIX_CONF_FOLDER, None, None, 0o0755, force).await?;
        let create_file = CreateFile::plan(NIX_CONF, None, None, 0o0664, buf, force).await?;
        Ok(Self {
            create_directory,
            create_file,
        }
        .into())
    }
}

#[async_trait::async_trait]
#[typetag::serde(name = "place_nix_configuration")]
impl Action for PlaceNixConfiguration {
    fn tracing_synopsis(&self) -> String {
        format!("Place the nix configuration in `{NIX_CONF}`")
    }

    fn execute_description(&self) -> Vec<ActionDescription> {
        vec![ActionDescription::new(
            self.tracing_synopsis(),
            vec![
                "This file is read by the Nix daemon to set its configuration options at runtime."
                    .to_string(),
            ],
        )]
    }

    #[tracing::instrument(skip_all)]
    async fn execute(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Self {
            create_file,
            create_directory,
        } = self;

        create_directory.try_execute().await?;
        create_file.try_execute().await?;

        Ok(())
    }

    fn revert_description(&self) -> Vec<ActionDescription> {
        vec![ActionDescription::new(
            format!("Remove the nix configuration in `{NIX_CONF}`"),
            vec![
                "This file is read by the Nix daemon to set its configuration options at runtime."
                    .to_string(),
            ],
        )]
    }

    #[tracing::instrument(skip_all)]
    async fn revert(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Self {
            create_file,
            create_directory,
        } = self;

        create_file.try_revert().await?;
        create_directory.try_revert().await?;

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PlaceNixConfigurationError {
    #[error("Creating file")]
    CreateFile(
        #[source]
        #[from]
        CreateFileError,
    ),
    #[error("Creating directory")]
    CreateDirectory(
        #[source]
        #[from]
        CreateDirectoryError,
    ),
}
