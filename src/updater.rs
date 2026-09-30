use anyhow::{Context, Result, bail};
use self_update::cargo_crate_version;
use std::{cmp::Ordering, env};

pub fn update() -> Result<()> {
    let current = env::current_exe().context("failed to locate the installed mcw executable")?;
    let tray = current.with_file_name("mcw-tray.exe");
    let mut cli_update = self_update::backends::github::Update::configure();
    cli_update
        .repo_owner("Roman-Cuisset")
        .repo_name("miccamwatch")
        .bin_name("mcw")
        .asset_identifier("windows-x86_64.zip")
        .current_version(cargo_crate_version!())
        .checksum_from_asset("SHA256SUMS")
        .show_download_progress(true)
        .no_confirm(true);

    let releases = cli_update
        .build()
        .context("failed to configure the updater")?
        .get_latest_release()
        .context("failed to check the latest release")?;
    let latest = releases
        .latest()
        .context("no MicCamWatch release is available")?;
    let version = latest.version();
    let comparison = self_update::version::cmp_versions(cargo_crate_version!(), version)
        .context("failed to compare MicCamWatch versions")?;
    let tray_is_current = tray.exists() && crate::autostart::tray_matches_current_version(&tray)?;
    if comparison == Ordering::Greater {
        if !tray_is_current {
            bail!(
                "mcw {} is newer than the latest release, but {} is missing or outdated; install matching binaries together",
                cargo_crate_version!(),
                tray.display()
            );
        }
        println!(
            "mcw {} and mcw-tray are already up to date.",
            cargo_crate_version!()
        );
        return Ok(());
    }
    if comparison == Ordering::Equal && tray_is_current {
        crate::autostart::refresh_if_enabled(version)
            .context("failed to refresh the enabled autostart registration")?;
        println!("mcw {version} and mcw-tray are already up to date.");
        return Ok(());
    }

    // Pin both downloads to the same release. The companion is installed first:
    // failure (including a running, locked tray) leaves the installed CLI intact
    // and cannot incorrectly report that both applications were updated.
    let tag = format!("v{version}");
    let tray_is_target = if comparison == Ordering::Equal {
        tray_is_current
    } else {
        tray.exists() && crate::autostart::tray_matches_version(&tray, version)?
    };
    if !tray_is_target {
        self_update::backends::github::Update::configure()
            .repo_owner("Roman-Cuisset")
            .repo_name("miccamwatch")
            .bin_name("mcw-tray")
            .bin_install_path(&tray)
            .asset_identifier("windows-x86_64.zip")
            .current_version("0.0.0")
            .release_tag(&tag)
            .checksum_from_asset("SHA256SUMS")
            .show_download_progress(true)
            .no_confirm(true)
            .build()
            .context("failed to configure the companion tray updater")?
            .update()
            .with_context(|| {
                format!(
                    "failed to install matching tray {}; stop the running tray and retry",
                    tray.display()
                )
            })?;
    }

    if comparison == Ordering::Less {
        cli_update
            .release_tag(&tag)
            .build()
            .context("failed to configure the pinned mcw update")?
            .update()
            .context(
                "the tray was installed, but mcw could not be updated; retry to finish the update",
            )?;
    }

    crate::autostart::refresh_if_enabled(version).context(
        "binaries updated, but the enabled autostart registration could not be refreshed",
    )?;
    if comparison == Ordering::Less {
        println!("mcw and mcw-tray updated to {version}.");
    } else {
        println!("mcw-tray repaired to match mcw {version}.");
    }
    Ok(())
}
