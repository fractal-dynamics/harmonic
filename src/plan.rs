use std::path::PathBuf;

use crossterm::style::Stylize;

use crate::{
    action::{Action, ActionDescription},
    planner::Planner,
    HarmonicError,
};

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct InstallPlan {
    pub(crate) actions: Vec<Box<dyn Action>>,

    pub(crate) planner: Box<dyn Planner>,
}

impl InstallPlan {
    #[tracing::instrument(skip_all)]
    pub fn describe_execute(
        &self,
        explain: bool,
    ) -> Result<String, Box<dyn std::error::Error + Sync + Send>> {
        let Self { planner, actions } = self;
        let buf = format!(
            "\
            Nix install plan\n\
            \n\
            Planner: {planner}\n\
            \n\
            Planner settings:\n\
            \n\
            {plan_settings}\n\
            \n\
            The following actions will be taken{maybe_explain}:\n\
            \n\
            {actions}\n\
        ",
            maybe_explain = if !explain {
                " (`--explain` for more context)"
            } else {
                ""
            },
            planner = planner.typetag_name(),
            plan_settings = planner
                .describe()?
                .into_iter()
                .map(|(k, v)| format!("* {k}: {v}", k = k.bold().white()))
                .collect::<Vec<_>>()
                .join("\n"),
            actions = actions
                .iter()
                .map(|v| v.describe_execute())
                .flatten()
                .map(|desc| {
                    let ActionDescription {
                        description,
                        explanation,
                    } = desc;

                    let mut buf = String::default();
                    buf.push_str(&format!("* {description}"));
                    if explain {
                        for line in explanation {
                            buf.push_str(&format!("\n  {line}"));
                        }
                    }
                    buf
                })
                .collect::<Vec<_>>()
                .join("\n"),
        );
        Ok(buf)
    }

    #[tracing::instrument(skip_all)]
    pub async fn install(&mut self) -> Result<(), HarmonicError> {
        let Self {
            actions,
            planner: _,
        } = self;

        // This is **deliberately sequential**.
        // Actions which are parallelizable are represented by "group actions" like CreateUsers
        // The plan itself represents the concept of the sequence of stages.
        for action in actions {
            if let Err(err) = action.execute().await {
                if let Err(err) = write_receipt(self.clone()).await {
                    tracing::error!("Error saving receipt: {:?}", err);
                }
                return Err(HarmonicError::ActionError(err));
            }
        }

        write_receipt(self.clone()).await?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub fn describe_revert(&self, explain: bool) -> String {
        let Self { planner, actions } = self;
        format!(
            "\
            This Nix uninstall is for:\n\
              Operating System: {os_type}\n\
              Init system: {init_type}\n\
              Nix channels: {nix_channels}\n\
            \n\
            Created by planner: {planner:?}
            \n\
            The following actions will be taken:\n\
            {actions}
        ",
            os_type = "Linux",
            init_type = "systemd",
            nix_channels = "todo",
            actions = actions
                .iter()
                .rev()
                .map(|v| v.describe_revert())
                .flatten()
                .map(|desc| {
                    let ActionDescription {
                        description,
                        explanation,
                    } = desc;

                    let mut buf = String::default();
                    buf.push_str(&format!("* {description}\n"));
                    if explain {
                        for line in explanation {
                            buf.push_str(&format!("  {line}\n"));
                        }
                    }
                    buf
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    #[tracing::instrument(skip_all)]
    pub async fn revert(&mut self) -> Result<(), HarmonicError> {
        let Self {
            actions,
            planner: _,
        } = self;

        // This is **deliberately sequential**.
        // Actions which are parallelizable are represented by "group actions" like CreateUsers
        // The plan itself represents the concept of the sequence of stages.
        for action in actions.iter_mut().rev() {
            if let Err(err) = action.revert().await {
                if let Err(err) = write_receipt(self.clone()).await {
                    tracing::error!("Error saving receipt: {:?}", err);
                }
                return Err(HarmonicError::ActionError(err));
            }
        }

        Ok(())
    }
}

async fn write_receipt(plan: InstallPlan) -> Result<(), HarmonicError> {
    tokio::fs::create_dir_all("/nix")
        .await
        .map_err(|e| HarmonicError::RecordingReceipt(PathBuf::from("/nix"), e))?;
    let install_receipt_path = PathBuf::from("/nix/receipt.json");
    let self_json =
        serde_json::to_string_pretty(&plan).map_err(HarmonicError::SerializingReceipt)?;
    tokio::fs::write(&install_receipt_path, self_json)
        .await
        .map_err(|e| HarmonicError::RecordingReceipt(install_receipt_path, e))?;
    Result::<(), HarmonicError>::Ok(())
}
