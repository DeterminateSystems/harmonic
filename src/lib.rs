pub mod action;
pub mod channel_value;
pub mod cli;
mod error;
mod interaction;
mod os;
mod plan;
pub mod planner;
mod settings;

use std::{ffi::OsStr, process::Output};

pub use action::Action;
pub use planner::Planner;

pub use error::HarmonicError;
pub use plan::InstallPlan;
use planner::BuiltinPlanner;

pub use settings::CommonSettings;

use tokio::process::Command;

#[tracing::instrument(skip_all, fields(command = %format!("{:?}", command.as_std())))]
async fn execute_command(command: &mut Command) -> Result<Output, std::io::Error> {
    // TODO(@hoverbear): When tokio releases past 1.21.2, add a process group https://github.com/DeterminateSystems/harmonic/issues/41#issuecomment-1309513073

    let command_str = format!("{:?}", command.as_std());
    tracing::trace!("Executing `{command_str}`");
    let output = command.output().await?;
    match output.status.success() {
        true => Ok(output),
        false => Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "Command `{command_str}` failed status, stderr:\n{}\n",
                String::from_utf8(output.stderr).unwrap_or_else(|_e| String::from("<Non-UTF-8>"))
            ),
        )),
    }
}

#[tracing::instrument(skip_all, fields(
    k = %k.as_ref().to_string_lossy(),
    v = %v.as_ref().to_string_lossy(),
))]
fn set_env(k: impl AsRef<OsStr>, v: impl AsRef<OsStr>) {
    tracing::trace!("Setting env");
    std::env::set_var(k.as_ref(), v.as_ref());
}

trait BoxableError: std::error::Error + Send + Sync {
    fn boxed(self) -> Box<dyn std::error::Error + Send + Sync>
    where
        Self: Sized + 'static,
    {
        Box::new(self)
    }
}

impl<E> BoxableError for E where E: std::error::Error + Send + Sized + Sync {}
