use std::io::Write;

use anyhow::Context;

use crate::utils::{get_config_value, list_config, set_config_value};

#[derive(clap::Args)]
pub struct Args {
    /// Get the value for a section/key pair
    #[arg(long, value_name = "KEY", exclusive = true)]
    pub get: Option<String>,

    /// List all keys and values
    #[arg(short, long, exclusive = true)]
    pub list: bool,

    /// Section and key, separated by a period
    #[arg(requires = "value")]
    pub key: Option<String>,

    /// New value to be stored
    pub value: Option<String>,
}

pub enum Action {
    Get(String),
    Set(String, String),
    List,
}

impl From<Args> for Action {
    fn from(
        Args {
            get,
            list,
            key,
            value,
        }: Args,
    ) -> Self {
        match (get, list, key, value) {
            (Some(key), _, _, _) => Self::Get(key),
            (None, true, _, _) => Self::List,
            (None, false, Some(key), Some(value)) => Self::Set(key, value),
            _ => unreachable!("clap ensures at least one is present"),
        }
    }
}

pub fn config(action: Action, mut output: impl Write) -> anyhow::Result<()> {
    match action {
        Action::Get(key) => {
            let (section, key) = key.split_once('.').context("key must contain a section")?;
            let value = get_config_value(section, key)?.context("no value found")?;
            output.write_all(value.as_bytes())?;
        }

        Action::Set(key, value) => {
            let (section, key) = key.split_once('.').context("key must contain a section")?;
            set_config_value(section, key, value)?;
        }

        Action::List => {
            let list = list_config()?;
            output.write_all(list.as_bytes())?;
        }
    }

    Ok(())
}
