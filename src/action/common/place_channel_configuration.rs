use crate::action::base::CreateFile;
use crate::action::ActionError;
use crate::action::{Action, ActionDescription, StatefulAction};
use crate::ChannelValue;
use tracing::{span, Span};

/**
Place a channel configuration containing `channels` to the `$ROOT_HOME/.nix-channels` file
 */
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct PlaceChannelConfiguration {
    channels: Vec<ChannelValue>,
    create_file: StatefulAction<CreateFile>,
}

impl PlaceChannelConfiguration {
    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn plan(
        channels: Vec<ChannelValue>,
        force: bool,
    ) -> Result<StatefulAction<Self>, ActionError> {
        let buf = channels
            .iter()
            .map(|ChannelValue(name, url)| format!("{} {}", url, name))
            .collect::<Vec<_>>()
            .join("\n");
        let create_file = CreateFile::plan(
            dirs::home_dir()
                .ok_or_else(|| {
                    ActionError::Custom(Box::new(PlaceChannelConfigurationError::NoRootHome))
                })?
                .join(".nix-channels"),
            None,
            None,
            0o0644,
            buf,
            force,
        )
        .await?;
        Ok(Self {
            create_file,
            channels,
        }
        .into())
    }
}

#[async_trait::async_trait]
#[typetag::serde(name = "place_channel_configuration")]
impl Action for PlaceChannelConfiguration {
    fn tracing_synopsis(&self) -> String {
        format!(
            "Place channel configuration at `{}`",
            self.create_file.inner().path.display()
        )
    }

    fn tracing_span(&self) -> Span {
        span!(
            tracing::Level::DEBUG,
            "place_channel_configuration",
            channels = self
                .channels
                .iter()
                .map(|ChannelValue(c, u)| format!("{c}={u}"))
                .collect::<Vec<_>>()
                .join(", "),
        )
    }

    fn execute_description(&self) -> Vec<ActionDescription> {
        vec![ActionDescription::new(self.tracing_synopsis(), vec![])]
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn execute(&mut self) -> Result<(), ActionError> {
        let Self {
            create_file,
            channels: _,
        } = self;

        create_file.try_execute().await?;

        Ok(())
    }

    fn revert_description(&self) -> Vec<ActionDescription> {
        vec![ActionDescription::new(
            format!(
                "Remove channel configuration at `{}`",
                self.create_file.inner().path.display()
            ),
            vec![],
        )]
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn revert(&mut self) -> Result<(), ActionError> {
        let Self {
            create_file,
            channels: _,
        } = self;

        create_file.try_revert().await?;

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PlaceChannelConfigurationError {
    #[error("No root home found to place channel configuration in")]
    NoRootHome,
}
