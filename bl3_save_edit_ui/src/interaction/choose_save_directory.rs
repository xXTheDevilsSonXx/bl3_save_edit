use std::path::PathBuf;

use anyhow::{Context, Result};
use native_dialog::{Dialog, OpenSingleDir};

pub async fn choose() -> Result<PathBuf> {
    let dialog = OpenSingleDir { dir: None };
    let res = dialog.show()?.context("no directory was selected")?;

    Ok(res)
}