use crate::CommonSettings;
use crate::{
    action::{
        base::{CreateGroup, CreateGroupError, CreateUser, CreateUserError},
        Action, ActionDescription, ActionImplementation, ActionState,
    },
    BoxableError,
};
use tokio::task::{JoinError, JoinSet};

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct CreateUsersAndGroup {
    daemon_user_count: usize,
    nix_build_group_name: String,
    nix_build_group_id: usize,
    nix_build_user_prefix: String,
    nix_build_user_id_base: usize,
    create_group: CreateGroup,
    create_users: Vec<CreateUser>,
    action_state: ActionState,
}

impl CreateUsersAndGroup {
    #[tracing::instrument(skip_all)]
    pub async fn plan(settings: CommonSettings) -> Result<Self, CreateUsersAndGroupError> {
        // TODO(@hoverbear): CHeck if it exist, error if so
        let create_group = CreateGroup::plan(
            settings.nix_build_group_name.clone(),
            settings.nix_build_group_id,
        );
        // TODO(@hoverbear): CHeck if they exist, error if so
        let create_users = (0..settings.daemon_user_count)
            .map(|count| {
                CreateUser::plan(
                    format!("{}{count}", settings.nix_build_user_prefix),
                    settings.nix_build_user_id_base + count,
                    settings.nix_build_group_name.clone(),
                    settings.nix_build_group_id,
                )
            })
            .collect();
        Ok(Self {
            daemon_user_count: settings.daemon_user_count,
            nix_build_group_name: settings.nix_build_group_name,
            nix_build_group_id: settings.nix_build_group_id,
            nix_build_user_prefix: settings.nix_build_user_prefix,
            nix_build_user_id_base: settings.nix_build_user_id_base,
            create_group,
            create_users,
            action_state: ActionState::Uncompleted,
        })
    }
}

#[async_trait::async_trait]
#[typetag::serde(name = "create_users_and_group")]
impl Action for CreateUsersAndGroup {
    fn tracing_synopsis(&self) -> String {
        format!(
            "Create build users (UID {}-{}) and group (GID {})",
            self.nix_build_user_id_base,
            self.nix_build_user_id_base + self.daemon_user_count,
            self.nix_build_group_id
        )
    }

    fn execute_description(&self) -> Vec<ActionDescription> {
        let Self {
            daemon_user_count: _,
            nix_build_group_name: _,
            nix_build_group_id: _,
            nix_build_user_prefix: _,
            nix_build_user_id_base: _,
            create_group,
            create_users,
            action_state: _,
        } = &self;

        let mut create_users_descriptions = Vec::new();
        for create_user in create_users {
            if let Some(val) = create_user.describe_execute().iter().next() {
                create_users_descriptions.push(val.description.clone())
            }
        }

        let mut explanation = vec![
            format!("The nix daemon requires system users (and a group they share) which it can act as in order to build"),
        ];
        if let Some(val) = create_group.describe_execute().iter().next() {
            explanation.push(val.description.clone())
        }
        explanation.append(&mut create_users_descriptions);

        vec![ActionDescription::new(self.tracing_synopsis(), explanation)]
    }

    #[tracing::instrument(skip_all, fields(
        daemon_user_count = self.daemon_user_count,
        nix_build_group_name = self.nix_build_group_name,
        nix_build_group_id = self.nix_build_group_id,
        nix_build_user_prefix = self.nix_build_user_prefix,
        nix_build_user_id_base = self.nix_build_user_id_base,
    ))]
    async fn execute(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Self {
            create_users,
            create_group,
            daemon_user_count: _,
            nix_build_group_name: _,
            nix_build_group_id: _,
            nix_build_user_prefix: _,
            nix_build_user_id_base: _,
            action_state: _,
        } = self;

        // Create group
        create_group.try_execute().await?;

        // Mac is apparently not threadsafe here...
        use target_lexicon::OperatingSystem;
        match OperatingSystem::host() {
            OperatingSystem::MacOSX {
                major: _,
                minor: _,
                patch: _,
            }
            | OperatingSystem::Darwin => {
                for create_user in create_users.iter_mut() {
                    create_user.try_execute().await?;
                }
            },
            _ => {
                let mut set = JoinSet::new();
                let mut errors: Vec<Box<dyn std::error::Error + Send + Sync>> = Vec::new();
                for (idx, create_user) in create_users.iter_mut().enumerate() {
                    let mut create_user_clone = create_user.clone();
                    let _abort_handle = set.spawn(async move {
                        create_user_clone.try_execute().await?;
                        Result::<_, _>::Ok((idx, create_user_clone))
                    });
                }

                while let Some(result) = set.join_next().await {
                    match result {
                        Ok(Ok((idx, success))) => create_users[idx] = success,
                        Ok(Err(e)) => errors.push(e),
                        Err(e) => return Err(e)?,
                    };
                }

                if !errors.is_empty() {
                    if errors.len() == 1 {
                        return Err(errors.into_iter().next().unwrap().into());
                    } else {
                        return Err(CreateUsersAndGroupError::CreateUsers(errors).boxed());
                    }
                }
            },
        };

        Ok(())
    }

    fn revert_description(&self) -> Vec<ActionDescription> {
        let Self {
            daemon_user_count: _,
            nix_build_group_name: _,
            nix_build_group_id: _,
            nix_build_user_prefix: _,
            nix_build_user_id_base: _,
            create_group,
            create_users,
            action_state: _,
        } = &self;
        if self.action_state == ActionState::Uncompleted {
            vec![]
        } else {
            let mut create_users_descriptions = Vec::new();
            for create_user in create_users {
                if let Some(val) = create_user.describe_revert().iter().next() {
                    create_users_descriptions.push(val.description.clone())
                }
            }

            let mut explanation = vec![
                format!("The nix daemon requires system users (and a group they share) which it can act as in order to build"),
            ];
            if let Some(val) = create_group.describe_revert().iter().next() {
                explanation.push(val.description.clone())
            }
            explanation.append(&mut create_users_descriptions);

            vec![ActionDescription::new(
                format!("Remove Nix users and group"),
                explanation,
            )]
        }
    }

    #[tracing::instrument(skip_all, fields(
        daemon_user_count = self.daemon_user_count,
        nix_build_group_name = self.nix_build_group_name,
        nix_build_group_id = self.nix_build_group_id,
        nix_build_user_prefix = self.nix_build_user_prefix,
        nix_build_user_id_base = self.nix_build_user_id_base,
    ))]
    async fn revert(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Self {
            create_users,
            create_group,
            daemon_user_count: _,
            nix_build_group_name: _,
            nix_build_group_id: _,
            nix_build_user_prefix: _,
            nix_build_user_id_base: _,
            action_state: _,
        } = self;
        let mut set = JoinSet::new();

        let mut errors = Vec::default();

        for (idx, create_user) in create_users.iter().enumerate() {
            let mut create_user_clone = create_user.clone();
            let _abort_handle = set.spawn(async move {
                create_user_clone.revert().await?;
                Result::<_, Box<dyn std::error::Error + Send + Sync>>::Ok((idx, create_user_clone))
            });
        }

        while let Some(result) = set.join_next().await {
            match result {
                Ok(Ok((idx, success))) => create_users[idx] = success,
                Ok(Err(e)) => errors.push(e),
                Err(e) => return Err(e.boxed())?,
            };
        }

        if !errors.is_empty() {
            if errors.len() == 1 {
                return Err(errors.into_iter().next().unwrap().into());
            } else {
                return Err(CreateUsersAndGroupError::CreateUsers(errors).boxed());
            }
        }

        // Create group
        create_group.revert().await?;

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
pub enum CreateUsersAndGroupError {
    #[error("Creating user")]
    CreateUser(
        #[source]
        #[from]
        CreateUserError,
    ),
    #[error("Multiple errors: {}", .0.iter().map(|v| {
        if let Some(source) = v.source() {
            format!("{v} ({source})")
        } else {
            format!("{v}") 
        }
    }).collect::<Vec<_>>().join(" & "))]
    CreateUsers(Vec<Box<dyn std::error::Error + Send + Sync>>),
    #[error("Creating group")]
    CreateGroup(
        #[source]
        #[from]
        CreateGroupError,
    ),
    #[error("Joining spawned async task")]
    Join(
        #[source]
        #[from]
        JoinError,
    ),
}
