use std::{collections::HashMap, io::Cursor};

use clap::ArgAction;
use tokio::process::Command;

use crate::{
    action::top_level::{
        darwin::{CreateApfsVolume, KickstartLaunchctlService},
        ConfigureNix, ProvisionNix,
    },
    execute_command,
    os::darwin::DiskUtilOutput,
    planner::{BuiltinPlannerError, Planner},
    BuiltinPlanner, CommonSettings, InstallPlan,
};

#[derive(Debug, Clone, clap::Parser, serde::Serialize, serde::Deserialize)]
pub struct DarwinMulti {
    #[clap(flatten)]
    pub settings: CommonSettings,
    #[clap(
        long,
        action(ArgAction::SetTrue),
        default_value = "false",
        env = "HARMONIC_VOLUME_ENCRYPT"
    )]
    pub volume_encrypt: bool,
    #[clap(long, default_value = "Nix Store", env = "HARMONIC_VOLUME_LABEL")]
    pub volume_label: String,
    #[clap(long, env = "HARMONIC_ROOT_DISK")]
    pub root_disk: Option<String>,
}

async fn default_root_disk() -> Result<String, BuiltinPlannerError> {
    let buf = execute_command(Command::new("/usr/sbin/diskutil").args(["info", "-plist", "/"]))
        .await
        .unwrap()
        .stdout;
    let the_plist: DiskUtilOutput = plist::from_reader(Cursor::new(buf))?;

    Ok(the_plist.parent_whole_disk)
}

#[async_trait::async_trait]
#[typetag::serde(name = "darwin-multi")]
impl Planner for DarwinMulti {
    async fn default() -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(Self {
            settings: CommonSettings::default()?,
            root_disk: Some(default_root_disk().await?),
            volume_encrypt: false,
            volume_label: "Nix Store".into(),
        })
    }

    async fn plan(
        mut self,
    ) -> Result<crate::InstallPlan, Box<dyn std::error::Error + Sync + Send>> {
        self.root_disk = match self.root_disk {
            root_disk @ Some(_) => root_disk,
            None => {
                let buf = execute_command(
                    Command::new("/usr/sbin/diskutil").args(["info", "-plist", "/"]),
                )
                .await
                .unwrap()
                .stdout;
                let the_plist: DiskUtilOutput = plist::from_reader(Cursor::new(buf)).unwrap();

                Some(the_plist.parent_whole_disk)
            },
        };

        let volume_label = "Nix Store".into();

        Ok(InstallPlan {
            planner: Box::new(self.clone()),
            actions: vec![
                // Create Volume step:
                //
                // setup_Synthetic -> create_synthetic_objects
                // Unmount -> create_volume -> Setup_fstab -> maybe encrypt_volume -> launchctl bootstrap -> launchctl kickstart -> await_volume -> maybe enableOwnership
                Box::new(
                    CreateApfsVolume::plan(
                        self.root_disk.unwrap(), /* We just ensured it was populated */
                        volume_label,
                        false,
                        None,
                    )
                    .await?,
                ),
                Box::new(ProvisionNix::plan(self.settings.clone()).await?),
                Box::new(ConfigureNix::plan(self.settings).await?),
                Box::new(
                    KickstartLaunchctlService::plan("system/org.nixos.nix-daemon".into()).await?,
                ),
            ],
        })
    }

    fn describe(
        &self,
    ) -> Result<HashMap<String, serde_json::Value>, Box<dyn std::error::Error + Sync + Send>> {
        let Self {
            settings,
            volume_encrypt,
            volume_label,
            root_disk,
        } = self;
        let mut map = HashMap::default();

        map.extend(settings.describe()?.into_iter());
        map.insert(
            "volume_encrypt".into(),
            serde_json::to_value(volume_encrypt)?,
        );
        map.insert("volume_label".into(), serde_json::to_value(volume_label)?);
        map.insert("root_disk".into(), serde_json::to_value(root_disk)?);

        Ok(map)
    }
}

impl Into<BuiltinPlanner> for DarwinMulti {
    fn into(self) -> BuiltinPlanner {
        BuiltinPlanner::DarwinMulti(self)
    }
}
