use std::collections::HashMap;

use crate::{
    action::{
        base::CreateDirectory,
        common::ProvisionNix,
        linux::{CreateSystemdSysext, StartSystemdUnit},
    },
    planner::Planner,
    Action, BuiltinPlanner, CommonSettings,
};

#[derive(Debug, Clone, clap::Parser, serde::Serialize, serde::Deserialize)]
pub struct SteamDeck {
    #[clap(flatten)]
    pub settings: CommonSettings,
}

#[async_trait::async_trait]
#[typetag::serde(name = "steam-deck")]
impl Planner for SteamDeck {
    async fn default() -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(Self {
            settings: CommonSettings::default()?,
        })
    }

    async fn plan(&self) -> Result<Vec<Box<dyn Action>>, Box<dyn std::error::Error + Sync + Send>> {
        Ok(vec![
            Box::new(CreateSystemdSysext::plan("/var/lib/extensions/nix").await?),
            Box::new(CreateDirectory::plan("/nix", None, None, 0o0755, true).await?),
            Box::new(ProvisionNix::plan(&self.settings.clone()).await?),
            Box::new(StartSystemdUnit::plan("nix-daemon.socket".into()).await?),
        ])
    }

    fn settings(
        &self,
    ) -> Result<HashMap<String, serde_json::Value>, Box<dyn std::error::Error + Sync + Send>> {
        let Self { settings } = self;
        let mut map = HashMap::default();

        map.extend(settings.describe()?.into_iter());

        Ok(map)
    }
}

impl Into<BuiltinPlanner> for SteamDeck {
    fn into(self) -> BuiltinPlanner {
        BuiltinPlanner::SteamDeck(self)
    }
}
