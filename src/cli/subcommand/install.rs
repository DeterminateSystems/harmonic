use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use crate::{
    action::ActionState,
    cli::{ensure_root, interaction, signal_channel, CommandExecute},
    error::HasExpectedErrors,
    plan::RECEIPT_LOCATION,
    planner::Planner,
    settings::CommonSettings,
    BuiltinPlanner, InstallPlan,
};
use clap::{ArgAction, Parser};
use eyre::{eyre, WrapErr};
use owo_colors::OwoColorize;

/// Execute an install (possibly using an existing plan)
///
/// To pass custom options, select a planner, for example `nix-installer install linux-multi --help`
#[derive(Debug, Parser)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Install {
    #[clap(
        long,
        env = "NIX_INSTALLER_NO_CONFIRM",
        action(ArgAction::SetTrue),
        default_value = "false",
        global = true
    )]
    pub no_confirm: bool,

    #[clap(flatten)]
    pub settings: CommonSettings,

    #[clap(
        long,
        env = "NIX_INSTALLER_EXPLAIN",
        action(ArgAction::SetTrue),
        default_value = "false",
        global = true
    )]
    pub explain: bool,
    #[clap(env = "NIX_INSTALLER_PLAN")]
    pub plan: Option<PathBuf>,

    #[clap(subcommand)]
    pub planner: Option<BuiltinPlanner>,
}

#[async_trait::async_trait]
impl CommandExecute for Install {
    #[tracing::instrument(level = "debug", skip_all)]
    async fn execute(self) -> eyre::Result<ExitCode> {
        let Self {
            no_confirm,
            plan,
            planner,
            settings,
            explain,
        } = self;

        ensure_root()?;

        let existing_receipt: Option<InstallPlan> = match Path::new(RECEIPT_LOCATION).exists() {
            true => {
                let install_plan_string = tokio::fs::read_to_string(&RECEIPT_LOCATION)
                    .await
                    .wrap_err("Reading plan")?;
                Some(serde_json::from_str(&install_plan_string)?)
            },
            false => None,
        };

        let mut install_plan = match (planner, plan) {
            (Some(planner), None) => {
                let chosen_planner: Box<dyn Planner> = planner.clone().boxed();

                match existing_receipt {
                    Some(existing_receipt) => {
                        if existing_receipt.planner.typetag_name() != chosen_planner.typetag_name() {
                            eprintln!("{}", format!("Found existing plan in `{RECEIPT_LOCATION}` which used a different planner, try uninstalling the existing install").red());
                            return Ok(ExitCode::FAILURE)
                        }
                        if existing_receipt.planner.settings().map_err(|e| eyre!(e))? != chosen_planner.settings().map_err(|e| eyre!(e))? {
                            eprintln!("{}", format!("Found existing plan in `{RECEIPT_LOCATION}` which used different planner settings, try uninstalling the existing install").red());
                            return Ok(ExitCode::FAILURE)
                        }
                        if existing_receipt.actions.iter().all(|v| v.state == ActionState::Completed) {
                            eprintln!("{}", format!("Found existing plan in `{RECEIPT_LOCATION}`, with the same settings, already completed, try uninstalling and reinstalling if Nix isn't working").red());
                            return Ok(ExitCode::FAILURE)
                        }
                        existing_receipt
                    } ,
                    None => {
                        planner.plan().await.map_err(|e| eyre!(e))?
                    },
                }
            },
            (None, Some(plan_path)) => {
                let install_plan_string = tokio::fs::read_to_string(&plan_path)
                .await
                .wrap_err("Reading plan")?;
                serde_json::from_str(&install_plan_string)?
            },
            (None, None) => {
                let builtin_planner = BuiltinPlanner::from_common_settings(settings)
                    .await
                    .map_err(|e| eyre::eyre!(e))?;
                match existing_receipt {
                    Some(existing_receipt) => {
                        if existing_receipt.planner.typetag_name() != builtin_planner.typetag_name() {
                            eprintln!("{}", format!("Found existing plan in `{RECEIPT_LOCATION}` which used a different planner, try uninstalling the existing install").red());
                            return Ok(ExitCode::FAILURE)
                        }
                        if existing_receipt.planner.settings().map_err(|e| eyre!(e))? != builtin_planner.settings().map_err(|e| eyre!(e))? {
                            eprintln!("{}", format!("Found existing plan in `{RECEIPT_LOCATION}` which used different planner settings, try uninstalling the existing install").red());
                            return Ok(ExitCode::FAILURE)
                        }
                        if existing_receipt.actions.iter().all(|v| v.state == ActionState::Completed) {
                            eprintln!("{}", format!("Found existing plan in `{RECEIPT_LOCATION}`, with the same settings, already completed, try uninstalling and reinstalling if Nix isn't working").red());
                            return Ok(ExitCode::FAILURE)
                        }
                        existing_receipt
                    },
                    None => {
                        let res = builtin_planner.plan().await;
                        match res {
                            Ok(plan) => plan,
                            Err(e) => {
                                if let Some(expected) = e.expected() {
                                    eprintln!("{}", expected.red());
                                    return Ok(ExitCode::FAILURE);
                                }
                                return Err(e.into())
                            }
                        }
                    },
                }
            },
            (Some(_), Some(_)) => return Err(eyre!("`--plan` conflicts with passing a planner, a planner creates plans, so passing an existing plan doesn't make sense")),
        };

        if !no_confirm {
            if !interaction::confirm(
                install_plan
                    .describe_install(explain)
                    .map_err(|e| eyre!(e))?,
                true,
            )
            .await?
            {
                interaction::clean_exit_with_message("Okay, didn't do anything! Bye!").await;
            }
        }

        let (tx, rx1) = signal_channel().await?;

        match install_plan.install(rx1).await {
            Err(err) => {
                if !no_confirm {
                    let mut was_expected = false;
                    if let Some(expected) = err.expected() {
                        was_expected = true;
                        eprintln!("{}", expected.red())
                    }
                    if !was_expected {
                        let error = eyre!(err).wrap_err("Install failure");
                        tracing::error!("{:?}", error);
                    };

                    eprintln!("{}", "Installation failure, offering to revert...".red());
                    if !interaction::confirm(
                        install_plan
                            .describe_uninstall(explain)
                            .map_err(|e| eyre!(e))?,
                        true,
                    )
                    .await?
                    {
                        interaction::clean_exit_with_message("Okay, didn't do anything! Bye!")
                            .await;
                    }
                    let rx2 = tx.subscribe();
                    let res = install_plan.uninstall(rx2).await;

                    if let Err(e) = res {
                        if let Some(expected) = e.expected() {
                            eprintln!("{}", expected.red());
                            return Ok(ExitCode::FAILURE);
                        }
                        return Err(e.into());
                    } else {
                        println!(
                            "\
                            {message}\n\
                            ",
                            message = "Partial Nix install was uninstalled successfully!"
                                .white()
                                .bold(),
                        );
                    }
                } else {
                    if let Some(expected) = err.expected() {
                        eprintln!("{}", expected.red());
                        return Ok(ExitCode::FAILURE);
                    }

                    let error = eyre!(err).wrap_err("Install failure");
                    return Err(error);
                }
            },
            Ok(_) => {
                println!(
                    "\
                    {success}\n\
                    To get started using Nix, open a new shell or run `{shell_reminder}`\n\
                    ",
                    success = "Nix was installed successfully!".green().bold(),
                    shell_reminder = match std::env::var("SHELL") {
                        Ok(val) if val.contains("fish") =>
                            ". /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.fish".bold(),
                        Ok(_) | Err(_) =>
                            ". /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh".bold(),
                    },
                );
            },
        }

        Ok(ExitCode::SUCCESS)
    }
}
