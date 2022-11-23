use crate::{
    action::{
        base::{CreateFile, CreateFileError, CreateOrAppendFile, CreateOrAppendFileError},
        darwin::{
            BootstrapVolume, BootstrapVolumeError, CreateSyntheticObjects,
            CreateSyntheticObjectsError, CreateVolume, CreateVolumeError, EnableOwnership,
            EnableOwnershipError, EncryptVolume, EncryptVolumeError, UnmountVolume,
            UnmountVolumeError,
        },
        Action, ActionDescription, ActionImplementation, ActionState,
    },
    BoxableError,
};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::process::Command;

pub const NIX_VOLUME_MOUNTD_DEST: &str = "/Library/LaunchDaemons/org.nixos.darwin-store.plist";

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct CreateApfsVolume {
    disk: PathBuf,
    name: String,
    case_sensitive: bool,
    encrypt: bool,
    create_or_append_synthetic_conf: CreateOrAppendFile,
    create_synthetic_objects: CreateSyntheticObjects,
    unmount_volume: UnmountVolume,
    create_volume: CreateVolume,
    create_or_append_fstab: CreateOrAppendFile,
    encrypt_volume: Option<EncryptVolume>,
    setup_volume_daemon: CreateFile,
    bootstrap_volume: BootstrapVolume,
    enable_ownership: EnableOwnership,
    action_state: ActionState,
}

impl CreateApfsVolume {
    #[tracing::instrument(skip_all)]
    pub async fn plan(
        disk: impl AsRef<Path>,
        name: String,
        case_sensitive: bool,
        encrypt: bool,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let disk = disk.as_ref();
        let create_or_append_synthetic_conf = CreateOrAppendFile::plan(
            "/etc/synthetic.conf",
            None,
            None,
            0o0655,
            "nix\n".into(), /* The newline is required otherwise it segfaults */
        )
        .await
        .map_err(|e| e.boxed())?;

        let create_synthetic_objects = CreateSyntheticObjects::plan().await?;

        let unmount_volume = UnmountVolume::plan(disk, name.clone()).await?;

        let create_volume = CreateVolume::plan(disk, name.clone(), case_sensitive).await?;

        let create_or_append_fstab = CreateOrAppendFile::plan(
            "/etc/fstab",
            None,
            None,
            0o0655,
            format!("NAME=\"{name}\" /nix apfs rw,noauto,nobrowse,suid,owners"),
        )
        .await
        .map_err(|e| e.boxed())?;

        let encrypt_volume = if encrypt {
            Some(EncryptVolume::plan(disk, &name).await?)
        } else {
            None
        };

        let name_with_qoutes = format!("\"{name}\"");
        let encrypted_command;
        let mount_command = if encrypt {
            encrypted_command = format!("/usr/bin/security find-generic-password -s {name_with_qoutes} -w |  /usr/sbin/diskutil apfs unlockVolume {name_with_qoutes} -mountpoint /nix -stdinpassphrase");
            vec!["/bin/sh", "-c", encrypted_command.as_str()]
        } else {
            vec![
                "/usr/sbin/diskutil",
                "mount",
                "-mountPoint",
                "/nix",
                name.as_str(),
            ]
        };
        // TODO(@hoverbear): Use plist lib we have in tree...
        let mount_plist = format!(
            "\
            <?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
            <!DOCTYPE plist PUBLIC \"-//Apple Computer//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
            <plist version=\"1.0\">\n\
            <dict>\n\
            <key>RunAtLoad</key>\n\
            <true/>\n\
            <key>Label</key>\n\
            <string>org.nixos.darwin-store</string>\n\
            <key>ProgramArguments</key>\n\
            <array>\n\
                {}\
            </array>\n\
            </dict>\n\
            </plist>\n\
        \
        ", mount_command.iter().map(|v| format!("<string>{v}</string>\n")).collect::<Vec<_>>().join("\n")
        );
        let setup_volume_daemon =
            CreateFile::plan(NIX_VOLUME_MOUNTD_DEST, None, None, None, mount_plist, false).await?;

        let bootstrap_volume = BootstrapVolume::plan(NIX_VOLUME_MOUNTD_DEST).await?;
        let enable_ownership = EnableOwnership::plan("/nix").await?;

        Ok(Self {
            disk: disk.to_path_buf(),
            name,
            case_sensitive,
            encrypt,
            create_or_append_synthetic_conf,
            create_synthetic_objects,
            unmount_volume,
            create_volume,
            create_or_append_fstab,
            encrypt_volume,
            setup_volume_daemon,
            bootstrap_volume,
            enable_ownership,
            action_state: ActionState::Uncompleted,
        })
    }
}

#[async_trait::async_trait]
#[typetag::serde(name = "create_apfs_volume")]
impl Action for CreateApfsVolume {
    fn tracing_synopsis(&self) -> String {
        format!(
            "Create an APFS volume `{}` on `{}`",
            self.name,
            self.disk.display()
        )
    }

    fn execute_description(&self) -> Vec<ActionDescription> {
        let Self {
            disk: _, name: _, ..
        } = &self;
        vec![ActionDescription::new(self.tracing_synopsis(), vec![])]
    }

    #[tracing::instrument(skip_all, fields(destination,))]
    async fn execute(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Self {
            disk: _,
            name: _,
            case_sensitive: _,
            encrypt: _,
            create_or_append_synthetic_conf,
            create_synthetic_objects,
            unmount_volume,
            create_volume,
            create_or_append_fstab,
            encrypt_volume,
            setup_volume_daemon,
            bootstrap_volume,
            enable_ownership,
            action_state: _,
        } = self;

        create_or_append_synthetic_conf.try_execute().await?;
        create_synthetic_objects.try_execute().await?;
        unmount_volume.try_execute().await.ok(); // We actually expect this may fail.
        create_volume.try_execute().await?;
        create_or_append_fstab.try_execute().await?;
        if let Some(encrypt_volume) = encrypt_volume {
            encrypt_volume.try_execute().await?;
        }
        setup_volume_daemon.try_execute().await?;

        bootstrap_volume.try_execute().await?;

        let mut retry_tokens: usize = 50;
        loop {
            tracing::trace!(%retry_tokens, "Checking for Nix Store existence");
            let status = Command::new("/usr/sbin/diskutil")
                .args(["info", "/nix"])
                .stderr(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .status()
                .await
                .map_err(|e| CreateApfsVolumeError::Command(e).boxed())?;
            if status.success() || retry_tokens == 0 {
                break;
            } else {
                retry_tokens = retry_tokens.saturating_sub(1);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        enable_ownership.try_execute().await?;

        Ok(())
    }

    fn revert_description(&self) -> Vec<ActionDescription> {
        let Self { disk, name, .. } = &self;
        vec![ActionDescription::new(
            format!("Remove the APFS volume `{name}` on `{}`", disk.display()),
            vec![format!(
                "Create a writable, persistent systemd system extension.",
            )],
        )]
    }

    #[tracing::instrument(skip_all, fields(disk, name))]
    async fn revert(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Self {
            disk: _,
            name: _,
            case_sensitive: _,
            encrypt: _,
            create_or_append_synthetic_conf,
            create_synthetic_objects,
            unmount_volume,
            create_volume,
            create_or_append_fstab,
            encrypt_volume,
            setup_volume_daemon,
            bootstrap_volume,
            enable_ownership,
            action_state: _,
        } = self;

        enable_ownership.try_revert().await?;
        bootstrap_volume.try_revert().await?;
        setup_volume_daemon.try_revert().await?;
        if let Some(encrypt_volume) = encrypt_volume {
            encrypt_volume.try_revert().await?;
        }
        create_or_append_fstab.try_revert().await?;

        unmount_volume.try_revert().await?;
        create_volume.try_revert().await?;

        // Purposefully not reversed
        create_or_append_synthetic_conf.try_revert().await?;
        create_synthetic_objects.try_revert().await?;

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
pub enum CreateApfsVolumeError {
    #[error(transparent)]
    CreateFile(#[from] CreateFileError),
    #[error(transparent)]
    DarwinBootstrapVolume(#[from] BootstrapVolumeError),
    #[error(transparent)]
    DarwinCreateSyntheticObjects(#[from] CreateSyntheticObjectsError),
    #[error(transparent)]
    DarwinCreateVolume(#[from] CreateVolumeError),
    #[error(transparent)]
    DarwinEnableOwnership(#[from] EnableOwnershipError),
    #[error(transparent)]
    DarwinEncryptVolume(#[from] EncryptVolumeError),
    #[error(transparent)]
    DarwinUnmountVolume(#[from] UnmountVolumeError),
    #[error(transparent)]
    CreateOrAppendFile(#[from] CreateOrAppendFileError),
    #[error("Failed to execute command")]
    Command(#[source] std::io::Error),
}
