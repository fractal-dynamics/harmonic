use std::{path::PathBuf, process::ExitCode};

use crate::InstallPlan;
use clap::{ArgAction, Parser};
use eyre::WrapErr;

use crate::{cli::CommandExecute, interaction};

/// Uninstall a previously installed Nix (only Harmonic done installs supported)
#[derive(Debug, Parser)]
pub struct Uninstall {
    #[clap(
        long,
        action(ArgAction::SetTrue),
        default_value = "false",
        global = true
    )]
    pub no_confirm: bool,
    #[clap(
        long,
        action(ArgAction::SetTrue),
        default_value = "false",
        global = true
    )]
    pub explain: bool,
    #[clap(default_value = "/nix/receipt.json")]
    pub receipt: PathBuf,
}

#[async_trait::async_trait]
impl CommandExecute for Uninstall {
    #[tracing::instrument(skip_all, fields())]
    async fn execute(self) -> eyre::Result<ExitCode> {
        let Self {
            no_confirm,
            receipt,
            explain,
        } = self;

        let install_receipt_string = tokio::fs::read_to_string(receipt)
            .await
            .wrap_err("Reading receipt")?;
        let mut plan: InstallPlan = serde_json::from_str(&install_receipt_string)?;

        if !no_confirm {
            if !interaction::confirm(plan.describe_revert(explain)).await? {
                interaction::clean_exit_with_message("Okay, didn't do anything! Bye!").await;
            }
        }

        plan.revert().await?;
        // TODO(@hoverbear): It would be so nice to catch errors and offer the user a way to keep going...
        //                   However that will require being able to link error -> step and manually setting that step as `Uncompleted`.

        Ok(ExitCode::SUCCESS)
    }
}
