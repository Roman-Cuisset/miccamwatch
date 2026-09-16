use anyhow::{Context, Result};
use self_update::cargo_crate_version;

pub fn update() -> Result<()> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner("Roman-Cuisset")
        .repo_name("miccamwatch")
        .bin_name("mcw")
        .asset_identifier("windows-x86_64")
        .current_version(cargo_crate_version!())
        .checksum_from_asset("SHA256SUMS")
        .show_download_progress(true)
        .no_confirm(true)
        .build()
        .context("failed to configure the updater")?
        .update()
        .context("failed to update mcw")?;

    if status.is_updated() {
        println!("mcw updated to {}.", status.version());
    } else {
        println!("mcw {} is already up to date.", status.version());
    }
    Ok(())
}
