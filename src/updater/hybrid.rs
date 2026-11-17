use super::{bundle, download, msi, portable, process, transaction::ProtectionReservation};
use crate::frontends::desktop::package::MANIFEST;
use crate::uninstall::windows::{
    RegisteredOwner, digest, ensure_owned, kernel_parent, ordinary_user, process_image,
    registered_owner,
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    env,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, IsTerminal, Read, Write},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const WORKER: &str = "maintenance.exe";
const PLAN: &str = "plan.json";
const MSI: &str = "miccamwatch-hybrid-windows-x86_64.msi";
const ZIP: &str = "miccamwatch-hybrid-windows-x86_64.zip";
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    schema: u32,
    active: PathBuf,
    old_version: String,
    old_hash: String,
    new_version: String,
    new_hash: String,
    kind: Kind,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "installation", deny_unknown_fields)]
enum Kind {
    Msi { old_code: String, new_code: String },
    Portable,
    Remove,
}

fn version_supported(version: &str) -> bool {
    let fields: Vec<_> = version.split('.').collect();
    fields.len() == 3
        && fields.iter().all(|part| {
            !part.is_empty()
                && part.len() <= 5
                && part.bytes().all(|b| b.is_ascii_digit())
                && (part.len() == 1 || !part.starts_with('0'))
                && part.parse::<u16>().is_ok()
        })
}

fn installation_path(executable: &Path) -> Result<PathBuf> {
    ensure!(
        executable
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("mcw.exe")),
        "run the installed mcw.exe for maintenance"
    );
    let parent = executable
        .parent()
        .context("installation directory missing")?;
    crate::uninstall::windows::safe_ancestors(parent)?;
    Ok(parent.canonicalize()?)
}

fn base(active: &Path) -> Result<PathBuf> {
    portable::transaction_root(active)?.context(
        "put a portable bundle in a dedicated directory below LocalAppData before updating",
    )
}

pub fn hold_runtime_lease() -> Result<Option<File>> {
    let executable = env::current_exe()?;
    let root = executable
        .parent()
        .context("installation directory missing")?;
    let Some(base) = portable::transaction_root(&root.canonicalize()?)? else {
        return Ok(None);
    };
    if !root.join(MANIFEST).try_exists()? && !base.try_exists()? {
        return Ok(None);
    }
    let file = portable::lock(&base, false)?;
    portable::check_pending(&base)?;
    Ok(Some(file))
}

pub(super) fn update() -> Result<()> {
    portable::require_real_profile()?;
    let current = env::current_exe()?;
    let root = installation_path(&current)?;
    let base = base(&root)?;
    let _base_directory =
        portable::pin_parent(base.parent().context("maintenance parent missing")?)?;
    let lock = portable::lock(&base, true)?;
    let _maintenance_directory = portable::open_owned_directory(&base)?;
    portable::check_pending(&base)?;
    let installed = match registered_owner(&root)? {
        Some(RegisteredOwner::Msi(code)) => {
            msi::verify_installed(&root, &code, VERSION)?;
            Some(code)
        }
        Some(RegisteredOwner::Managed) => {
            bail!("use the registered installation manager for this update")
        }
        None => {
            ensure!(
                !crate::uninstall::windows::owner_guidance(&current, |message| eprintln!(
                    "{message}"
                ))?,
                "follow the detected package-manager guidance"
            );
            verify_portable_owner(&root)?;
            require_cwd_outside(&root)?;
            None
        }
    };
    let old_hash = bundle::verify(&root, VERSION, true)?;
    require_idle(&root)?;
    let releases = self_update::backends::github::Update::configure()
        .repo_owner("Roman-Cuisset")
        .repo_name("miccamwatch")
        .bin_name("mcw")
        .current_version(VERSION)
        .timeout(Duration::from_secs(20))
        .retries(0)
        .build()?
        .get_latest_release()
        .context("cannot retrieve authenticated MicCamWatch release metadata")?;
    let release = releases
        .latest()
        .context("no public MicCamWatch release found")?;
    let version = release.version();
    ensure!(
        version_supported(version)
            && release.release_notes_url().and_then(|url| url
                .strip_prefix("https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v"))
                == Some(version),
        "release identity or version is incompatible"
    );
    match self_update::version::cmp_versions(VERSION, version)? {
        Ordering::Equal => {
            println!("MicCamWatch {VERSION} is already installed.");
            return Ok(());
        }
        Ordering::Greater => bail!("refusing to downgrade MicCamWatch {VERSION} to {version}"),
        Ordering::Less => {}
    }
    let temporary = portable::private_job(&base)?;
    let job = temporary.path().to_owned();
    let scratch = portable::open_owned_directory(&job)?;
    let name = if installed.is_some() { MSI } else { ZIP };
    for asset in release
        .assets()
        .iter()
        .filter(|asset| matches!(asset.name(), "SHA256SUMS") || asset.name() == name)
    {
        ensure!(
            official_asset_url(asset.download_url()),
            "asset provenance cannot be verified"
        );
    }
    let (artifact, mut new_hash) = download::download_verified(release, name, &job)?;
    fs::remove_file(job.join("SHA256SUMS"))?;
    let kind = if let Some(old_code) = installed {
        let new_code = msi::inspect_candidate(&artifact, version)?;
        ensure!(
            !new_code.eq_ignore_ascii_case(&old_code),
            "MSI major upgrade reused the installed ProductCode"
        );
        msi::require_new_product(&new_code)?;
        Kind::Msi { old_code, new_code }
    } else {
        let candidate = job.join("candidate");
        new_hash = bundle::extract(bundle::pin(&artifact)?, &candidate, version)?;
        bundle::verify_flutter_version(&candidate, version)?;
        fs::remove_file(artifact)?;
        Kind::Portable
    };
    confirm(
        &format!("Apply verified MicCamWatch {version} ({name})?"),
        "update",
    )?;
    drop(scratch);
    let (job, fingerprint) = prepare_worker(
        temporary,
        &current,
        &Plan {
            schema: 1,
            active: root,
            old_version: VERSION.to_owned(),
            old_hash,
            new_version: version.to_owned(),
            new_hash,
            kind,
        },
    )?;
    drop(lock);
    hand_off(&job, &fingerprint)
}

fn verify_portable_owner(root: &Path) -> Result<()> {
    ordinary_user()?;
    let reserved = portable::local_app_data()?
        .join("Programs")
        .join("MicCamWatch");
    ensure!(
        !root
            .ancestors()
            .any(|path| path.as_os_str().eq_ignore_ascii_case(reserved.as_os_str())),
        "this directory is reserved for Windows Installer"
    );
    if root.try_exists()? {
        portable::open_owned_directory(root)?;
        ensure!(
            registered_owner(root)?.is_none(),
            "portable maintenance cannot alter a registered product"
        );
    }
    Ok(())
}

fn require_cwd_outside(root: &Path) -> Result<()> {
    ensure!(
        !env::current_dir()?.canonicalize()?.starts_with(root),
        "run the portable maintenance command from outside the installation directory"
    );
    Ok(())
}

pub fn remove_portable() -> Result<u8> {
    portable::require_real_profile()?;
    let current = env::current_exe()?;
    let root = installation_path(&current)?;
    verify_portable_owner(&root)?;
    require_cwd_outside(&root)?;
    let base = base(&root)?;
    let lock = portable::lock(&base, true)?;
    portable::check_pending(&base)?;
    require_idle(&root)?;
    crate::autostart::require_unregistered_for_portable_removal(&current)?;
    let old_hash = bundle::verify(&root, VERSION, true)?;
    confirm(
        &format!(
            "Remove the verified MicCamWatch bundle at {}?",
            root.display()
        ),
        "uninstall",
    )?;
    let plan = Plan {
        schema: 1,
        active: root,
        old_version: VERSION.to_owned(),
        old_hash,
        new_version: VERSION.to_owned(),
        new_hash: String::new(),
        kind: Kind::Remove,
    };
    let temporary = portable::private_job(&base)?;
    let (job, fingerprint) = prepare_worker(temporary, &current, &plan)?;
    drop(lock);
    hand_off(&job, &fingerprint)?;
    Ok(0)
}

fn prepare_worker(
    temporary: tempfile::TempDir,
    current: &Path,
    plan: &Plan,
) -> Result<(PathBuf, String)> {
    let job = temporary.path();
    let directory = portable::open_owned_directory(job)?;
    fs::copy(current, job.join(WORKER))?;
    portable::write_record(job, PLAN, &serde_json::to_vec(plan)?)?;
    let fingerprint = digest(&mut bundle::pin(&job.join(PLAN))?)?;
    drop(directory);
    Ok((temporary.keep(), fingerprint))
}

fn official_asset_url(url: &str) -> bool {
    url.strip_prefix("https://api.github.com/repos/Roman-Cuisset/miccamwatch/releases/assets/")
        .is_some_and(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()))
}

fn confirm(message: &str, keyword: &str) -> Result<()> {
    ensure!(
        std::io::stdin().is_terminal() && std::io::stdout().is_terminal(),
        "run MicCamWatch maintenance from an interactive terminal to approve the operation"
    );
    print!("{message} Type '{keyword}' to confirm: ");
    std::io::stdout().flush()?;
    let mut response = String::new();
    std::io::stdin().lock().take(32).read_line(&mut response)?;
    ensure!(response.trim() == keyword, "maintenance cancelled");
    Ok(())
}

fn hand_off(job: &Path, fingerprint: &str) -> Result<()> {
    let log = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(job.join("worker.log"))?;
    let mut child = Command::new(job.join(WORKER))
        .arg("__update-worker")
        .arg(std::process::id().to_string())
        .arg(crate::windows_control::current_process_created()?.to_string())
        .arg(fingerprint)
        .current_dir(job)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(log)
        .creation_flags(0x0800_0000)
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .context("maintenance acknowledgement channel missing")?;
    let (send, received) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(stdout.take(32))
            .read_line(&mut line)
            .map(|_| line);
        let _ = send.send(result);
    });
    let done = received
        .recv_timeout(Duration::from_secs(90))
        .context("maintenance acknowledgement timed out")??;
    ensure!(
        done == "ready\n",
        "maintenance refused, see {}",
        job.join("worker.log").display()
    );
    println!(
        "Maintenance accepted. Wait for completion in {}.",
        job.join("result.json").display()
    );
    Ok(())
}

fn read_plan(job: &Path, expected: Option<&str>) -> Result<Plan> {
    let mut input = bundle::pin(&job.join(PLAN))?;
    ensure_owned(&input)?;
    let mut bytes = Vec::new();
    (&mut input).take(8193).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 8192, "oversized maintenance plan");
    if let Some(expected) = expected {
        ensure!(
            digest(&mut input)? == expected,
            "maintenance plan changed after confirmation"
        );
    }
    let plan: Plan = serde_json::from_slice(&bytes)?;
    let valid_target = match plan.kind {
        Kind::Remove => plan.new_version == VERSION && plan.new_hash.is_empty(),
        _ => {
            version_supported(&plan.new_version)
                && self_update::version::cmp_versions(&plan.old_version, &plan.new_version)?
                    == Ordering::Less
        }
    };
    ensure!(
        plan.schema == 1 && plan.old_version == VERSION && valid_target,
        "incompatible maintenance plan"
    );
    Ok(plan)
}

fn read_context(expected: Option<&str>) -> Result<(PathBuf, Plan, File, File)> {
    let current = env::current_exe()?;
    let job = current
        .parent()
        .context("worker stage missing")?
        .canonicalize()?;
    ensure!(
        current.file_name().is_some_and(|name| name == WORKER)
            && job
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("update-")),
        "start the worker in its exact staged directory"
    );
    let directory = portable::open_owned_directory(&job)?;
    let plan = read_plan(&job, expected)?;
    let base = base(&plan.active)?;
    ensure!(
        job.parent() == Some(base.as_path()),
        "maintenance plan is outside its exact installation"
    );
    let lock = portable::lock(&base, true)?;
    ensure!(
        !job.join("committed").try_exists()? && !job.join("rolled-back").try_exists()?,
        "maintenance job already completed"
    );
    Ok((job, plan, lock, directory))
}

fn require_idle(active: &Path) -> Result<()> {
    ensure!(
        process::installed_running_tray(&active.join("mcw.exe"), &active.join("mcw-tray.exe"))?
            .is_none(),
        "exit the MicCamWatch tray normally before updating"
    );
    Ok(())
}

fn binaries(job: &Path, active: &Path, version: &str) -> Result<()> {
    bundle::verify_flutter_version(active, version)?;
    let scratch = tempfile::tempdir_in(job)?;
    super::validate_reported_version(
        &active.join("mcw.exe"),
        &format!("mcw {version}"),
        scratch.path(),
    )?;
    let tray_scratch = tempfile::tempdir_in(job)?;
    super::validate_reported_version(
        &active.join("mcw-tray.exe"),
        &format!("mcw-tray {version}"),
        tray_scratch.path(),
    )
}

fn swap<'a>(job: &'a Path, plan: &'a Plan) -> portable::Swap<'a> {
    portable::Swap {
        active: &plan.active,
        job,
        old_version: &plan.old_version,
        old_hash: &plan.old_hash,
        new_version: &plan.new_version,
        new_hash: &plan.new_hash,
    }
}

pub fn run_worker(pid: u32, created: u64, fingerprint: &str) -> Result<u8> {
    portable::require_real_profile()?;
    ensure!(
        kernel_parent()? == pid,
        "maintenance caller does not match kernel parent"
    );
    let identity = crate::windows_control::current_identity()?;
    let parent = crate::windows_control::invoker_lease(&identity, pid, created)?;
    let caller = process_image(pid)?;
    let (job, plan, _maintenance, _directory) = read_context(Some(fingerprint))?;
    ensure!(
        installation_path(&caller)? == plan.active,
        "maintenance caller is from another installation"
    );
    ensure!(
        digest(&mut bundle::pin(&caller)?)? == digest(&mut bundle::pin(&env::current_exe()?)?)?,
        "maintenance worker differs from the authenticated installed executable"
    );
    let result = (|| {
        require_idle(&plan.active)?;
        ensure!(
            bundle::verify(&plan.active, VERSION, true)? == plan.old_hash,
            "active package changed after confirmation"
        );
        let _protection = ProtectionReservation::acquire()?;
        let artifact = job.join(MSI);
        let pinned = match &plan.kind {
            Kind::Msi { old_code, new_code } => {
                msi::verify_installed(&plan.active, old_code, VERSION)?;
                msi::require_new_product(new_code)?;
                let mut file = bundle::pin(&artifact)?;
                ensure!(
                    digest(&mut file)? == plan.new_hash
                        && msi::inspect_candidate(&artifact, &plan.new_version)? == *new_code,
                    "MSI bytes or identity changed after confirmation"
                );
                Some(file)
            }
            Kind::Portable | Kind::Remove => {
                verify_portable_owner(&plan.active)?;
                if matches!(plan.kind, Kind::Remove) {
                    crate::autostart::require_unregistered_for_portable_removal(&caller)?;
                }
                None
            }
        };
        ensure!(
            !parent.exited()?,
            "confirmed maintenance caller already exited"
        );
        // Release the mapped installed CLI only after the worker has acquired
        // all guards. No native process is stopped by this handshake.
        {
            let mut acknowledgement = std::io::stdout().lock();
            acknowledgement.write_all(b"ready\n")?;
            acknowledgement.flush()?;
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        while !parent.exited()? {
            ensure!(
                Instant::now() < deadline,
                "updating CLI remained active; no changes applied"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        match &plan.kind {
            Kind::Msi { old_code, new_code } => {
                let _artifact = pinned.context("verified MSI handle missing")?;
                msi::install(
                    &artifact,
                    &job,
                    &plan.active,
                    old_code,
                    new_code,
                    VERSION,
                    &plan.new_version,
                )
                .and_then(|()| binaries(&job, &plan.active, &plan.new_version))
                .and_then(|()| portable::finish(&job, "committed"))
            }
            Kind::Portable => {
                swap(&job, &plan).apply(|| binaries(&job, &plan.active, &plan.new_version))
            }
            Kind::Remove => swap(&job, &plan).remove(),
        }
    })();
    if result.is_err()
        && !job.join("committed").try_exists()?
        && !job.join("rolled-back").try_exists()?
        && bundle::verify(&plan.active, VERSION, true).is_ok_and(|hash| hash == plan.old_hash)
        && match &plan.kind {
            Kind::Msi { old_code, .. } => {
                msi::require_restart_completed(&job).is_ok()
                    && msi::verify_installed(&plan.active, old_code, VERSION).is_ok()
            }
            Kind::Portable | Kind::Remove => !job.join("previous").try_exists()?,
        }
    {
        portable::finish(&job, "rolled-back")?;
    }
    let report = match &result {
        Ok(()) if matches!(plan.kind, Kind::Remove) => {
            serde_json::json!({"status":"removed","retained_bundle":job.join("previous").try_exists()?})
        }
        Ok(()) => serde_json::json!({"status":"installed","version":plan.new_version}),
        Err(error) => {
            let status = if matches!(msi::exit_code(&job)?, Some(1641 | 3010)) {
                "restart_required"
            } else if job.join("rolled-back").try_exists()? {
                "previous_bundle_verified"
            } else {
                "intervention_required"
            };
            serde_json::json!({"status":status,"error":format!("{error:#}")})
        }
    };
    portable::write_record(&job, "result.json", &serde_json::to_vec(&report)?)?;
    result?;
    Ok(0)
}

pub fn recover() -> Result<u8> {
    portable::require_real_profile()?;
    let (job, plan, _lock, _directory) = read_context(None)?;
    let _protection = ProtectionReservation::acquire()?;
    match &plan.kind {
        Kind::Portable | Kind::Remove => {
            verify_portable_owner(&plan.active)?;
            if plan.active.try_exists()? {
                require_idle(&plan.active)?;
            }
            swap(&job, &plan).recover()?;
            println!("The previous portable bundle was verified and restored.");
        }
        Kind::Msi { old_code, new_code } => {
            msi::require_restart_completed(&job)?;
            if msi::verify_installed(&plan.active, old_code, VERSION).is_ok() {
                ensure!(
                    bundle::verify(&plan.active, VERSION, true)? == plan.old_hash,
                    "previous MSI bundle differs"
                );
                portable::finish(&job, "rolled-back")?;
                println!("The previous MSI product and bundle are intact.");
            } else {
                msi::verify_installed(&plan.active, new_code, &plan.new_version)?;
                bundle::verify(&plan.active, &plan.new_version, true)?;
                binaries(&job, &plan.active, &plan.new_version)?;
                portable::finish(&job, "committed")?;
                println!("The installed MSI upgrade was verified.");
            }
        }
    }
    Ok(0)
}
