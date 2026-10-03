use anyhow::{Context, Result, bail};
use self_update::{
    ArchiveKind, Checksum, Download, Extract,
    update::{Release, ReleaseAsset},
};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};

const PACKAGE: &str = "miccamwatch-windows-x86_64.zip";
const SUMS: &str = "SHA256SUMS";

struct HexDigest<'a>(&'a [u8]);
impl std::fmt::Display for HexDigest<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}
/// Asset URLs and digests are borrowed from the one selected API response. No
/// tag/latest lookup occurs here, even when repairing a same-version companion.
pub(super) fn stage_package(release: &Release, stage: &Path) -> Result<()> {
    let package = exact_asset(release, PACKAGE)?;
    let sums = exact_asset(release, SUMS)?;
    let api_checksum = required_digest(package)?;
    let sums_path = stage.join(SUMS);
    transfer(sums, &sums_path, false)?;
    if sums.digest().is_some() {
        verify(&sums_path, &required_digest(sums)?)?;
    }
    let sums_text = fs::read_to_string(&sums_path)
        .context("release SHA256SUMS is not readable UTF-8; installation unchanged")?;
    let published_checksum = exact_sums_checksum(&sums_text, PACKAGE)?;
    // Require agreement before spending a package transfer. Both checks remain
    // pinned even if the mutable release's URLs serve different bytes later.
    if !sha256_hex(&api_checksum)?.eq_ignore_ascii_case(sha256_hex(&published_checksum)?) {
        bail!(
            "selected release API digest and SHA256SUMS disagree for {PACKAGE}; installation unchanged"
        );
    }
    let archive = stage.join(PACKAGE);
    transfer(package, &archive, true)?;
    verify(&archive, &api_checksum)?;
    let mut extract = Extract::from_source(&archive);
    extract.archive(ArchiveKind::Zip);
    for name in ["mcw.exe", "mcw-tray.exe"] {
        extract.extract_file(stage, name).map_err(|error| {
            transport_error(
                "extract verified release executable",
                &stage.join(name),
                error,
            )
        })?;
    }
    Ok(())
}

fn exact_asset<'a>(release: &'a Release, name: &str) -> Result<&'a ReleaseAsset> {
    let mut matches = release.assets().iter().filter(|asset| asset.name() == name);
    let asset = matches.next().with_context(|| {
        format!("selected release has no exact asset named {name}; installation unchanged")
    })?;
    if matches.next().is_some() {
        bail!("selected release has ambiguous duplicate asset {name}; installation unchanged");
    }
    Ok(asset)
}

fn required_digest(asset: &ReleaseAsset) -> Result<Checksum> {
    let digest = asset.digest().with_context(|| {
        format!(
            "selected release asset {} has no API digest; installation unchanged",
            asset.name()
        )
    })?;
    let checksum = Checksum::parse_digest(digest)?;
    sha256_hex(&checksum)?;
    Ok(checksum)
}

fn sha256_hex(checksum: &Checksum) -> Result<&str> {
    match checksum {
        Checksum::Sha256(hex) if hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()) => {
            Ok(hex)
        }
        _ => bail!(
            "release digest must be exactly 64 SHA-256 hexadecimal characters; installation unchanged"
        ),
    }
}

fn exact_sums_checksum(sums: &str, name: &str) -> Result<Checksum> {
    // SHA256SUMS is a named multi-asset manifest, not an unnamed .sha256 file.
    // The dependency parser also accepts basename aliases and a bare digest;
    // reject those ambiguous associations before reusing its checksum parser.
    let mut found = None;
    for line in sums
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let entry = if let Some(rest) = line.strip_prefix("SHA256 (") {
            rest.split_once(") = ").map(|(file, hex)| (hex, file))
        } else {
            line.find(char::is_whitespace).map(|split| {
                let (hex, file) = line.split_at(split);
                (
                    hex,
                    file.trim_start()
                        .strip_prefix('*')
                        .unwrap_or(file.trim_start()),
                )
            })
        };
        if let Some((_, file)) = entry
            && file == name
            && found.replace(line).is_some()
        {
            bail!("SHA256SUMS has duplicate entries for {name}; installation unchanged");
        }
    }
    let entry = found.with_context(|| {
        format!("SHA256SUMS has no exact named entry for {name}; installation unchanged")
    })?;
    let checksum = Checksum::from_sums_file(entry, name)?;
    sha256_hex(&checksum)?;
    Ok(checksum)
}

fn transfer(asset: &ReleaseAsset, path: &Path, progress: bool) -> Result<()> {
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            super::transaction::io_error("create staged release asset", path, error)
        })?;
    Download::from_url(asset.download_url())
        .request_header(
            self_update::http::header::ACCEPT,
            "application/octet-stream",
        )
        .show_download_progress(progress)
        .download_to(&file)
        .map_err(|error| transport_error("download selected release asset", path, error))?;
    file.sync_all()
        .map_err(|error| super::transaction::io_error("flush staged release asset", path, error))
}

fn verify(path: &Path, checksum: &Checksum) -> Result<()> {
    // self_update 1.3's Checksum::verify is private; Download has no checksum
    // hook. Stream once rather than read the archive into an allocated buffer.
    let mut file = fs::File::open(path)
        .map_err(|error| super::transaction::io_error("open staged checksum input", path, error))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            super::transaction::io_error("read staged checksum input", path, error)
        })?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    let computed = hash.finalize();
    let expected = sha256_hex(checksum)?;
    let hex_digit = |byte: u8| match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => unreachable!("sha256_hex validates hexadecimal characters"),
    };
    if !computed
        .iter()
        .zip(expected.as_bytes().as_chunks::<2>().0.iter())
        .all(|(actual, pair)| *actual == (hex_digit(pair[0]) << 4 | hex_digit(pair[1])))
    {
        bail!(
            "Release ZIP SHA-256 mismatch: expected {expected}, computed {}",
            HexDigest(computed.as_slice())
        );
    }
    Ok(())
}

fn transport_error(action: &str, path: &Path, error: self_update::errors::Error) -> anyhow::Error {
    match error {
        self_update::errors::Error::Io(error) => super::transaction::io_error(action, path, error),
        error => anyhow::Error::new(error).context(format!(
            "{action} failed for {}; installation unchanged",
            path.display()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;
    use std::{
        io::{Cursor, Write},
        net::TcpListener,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread::{self, JoinHandle},
        time::Duration,
    };

    struct Server {
        base: String,
        requests: Arc<Mutex<Vec<String>>>,
        stop: Arc<AtomicBool>,
        thread: Option<JoinHandle<()>>,
    }

    impl Server {
        fn new(package: Vec<u8>, sums: String) -> Result<Self> {
            let listener = TcpListener::bind("127.0.0.1:0")?;
            let base = format!("http://{}", listener.local_addr()?);
            listener.set_nonblocking(true)?;
            let requests = Arc::new(Mutex::new(Vec::new()));
            let stop = Arc::new(AtomicBool::new(false));
            let seen = Arc::clone(&requests);
            let stopping = Arc::clone(&stop);
            let thread = thread::spawn(move || {
                while !stopping.load(Ordering::Relaxed) {
                    let (mut stream, _) = match listener.accept() {
                        Ok(connection) => connection,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                            continue;
                        }
                        Err(error) => panic!("fixture accept failed: {error}"),
                    };
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut request = Vec::new();
                    let mut byte = [0u8; 1];
                    while request.len() < 8192 && !request.ends_with(b"\r\n\r\n") {
                        if stream.read(&mut byte).unwrap() == 0 {
                            break;
                        }
                        request.push(byte[0]);
                    }
                    let request = String::from_utf8(request).unwrap();
                    let path = request.split_whitespace().nth(1).unwrap().to_owned();
                    seen.lock().push(path.clone());
                    let body: &[u8] = match path.as_str() {
                        "/package" => &package,
                        "/sums" => sums.as_bytes(),
                        _ => panic!("unexpected release lookup or asset request: {path}"),
                    };
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .unwrap();
                    stream.write_all(body).unwrap();
                }
            });
            Ok(Self {
                base,
                requests,
                stop,
                thread: Some(thread),
            })
        }

        fn release(&self, digest: Option<&str>, package_name: &str) -> Result<Release> {
            let mut asset = ReleaseAsset::new(package_name, format!("{}/package", self.base));
            if let Some(digest) = digest {
                asset = asset.with_digest(format!("sha256:{digest}"));
            }
            Ok(Release::builder()
                .version(env!("CARGO_PKG_VERSION"))
                .assets([
                    asset,
                    ReleaseAsset::new(SUMS, format!("{}/sums", self.base)),
                ])
                .build()?)
        }
    }

    impl Drop for Server {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            self.thread.take().unwrap().join().unwrap();
        }
    }

    fn archive(names: &[&str]) -> Result<Vec<u8>> {
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for name in names {
            archive.start_file(
                *name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )?;
            archive.write_all(name.as_bytes())?;
        }
        Ok(archive.finish()?.into_inner())
    }

    fn digest(bytes: &[u8]) -> String {
        format!("{}", HexDigest(Sha256::digest(bytes).as_slice()))
    }

    #[test]
    fn selected_package_transfers_once_and_extracts_both_exact_binaries() -> Result<()> {
        let bytes = archive(&["mcw.exe", "mcw-tray.exe"])?;
        let digest = digest(&bytes);
        let server = Server::new(bytes, format!("{digest}  {PACKAGE}\n"))?;
        let stage = tempfile::tempdir()?;
        stage_package(&server.release(Some(&digest), PACKAGE)?, stage.path())?;
        assert_eq!(*server.requests.lock(), ["/sums", "/package"]);
        Ok(())
    }

    #[test]
    fn missing_api_digest_or_wrong_asset_name_makes_no_transfer() -> Result<()> {
        let server = Server::new(Vec::new(), String::new())?;
        for (digest, name) in [
            (None, PACKAGE),
            (Some("00"), "other-windows-x86_64.zip"),
            (Some("not-hex"), PACKAGE),
        ] {
            let stage = tempfile::tempdir()?;
            assert!(stage_package(&server.release(digest, name)?, stage.path()).is_err());
            assert!(!stage.path().join("mcw.exe").exists());
        }
        assert!(server.requests.lock().is_empty());
        Ok(())
    }

    #[test]
    fn named_sums_entry_is_required_and_pinned_to_api_digest() -> Result<()> {
        let bytes = archive(&["mcw.exe", "mcw-tray.exe"])?;
        let digest = digest(&bytes);
        for sums in [
            digest.clone(),
            format!("{digest}  wrong.zip"),
            format!("{digest}  dist/{PACKAGE}"),
            format!("{digest}  {PACKAGE}\n{digest}  {PACKAGE}"),
            format!("{}  {PACKAGE}", "0".repeat(64)),
        ] {
            let server = Server::new(bytes.clone(), sums)?;
            let stage = tempfile::tempdir()?;
            assert!(stage_package(&server.release(Some(&digest), PACKAGE)?, stage.path()).is_err());
            assert!(!stage.path().join(PACKAGE).exists());
            assert_eq!(*server.requests.lock(), ["/sums"]);
        }
        Ok(())
    }

    #[test]
    fn mutated_download_is_rejected_before_any_extraction() -> Result<()> {
        let original = archive(&["mcw.exe", "mcw-tray.exe"])?;
        let digest = digest(&original);
        let server = Server::new(
            original[..original.len() / 2].to_vec(),
            format!("{digest}  {PACKAGE}"),
        )?;
        let stage = tempfile::tempdir()?;
        assert!(stage_package(&server.release(Some(&digest), PACKAGE)?, stage.path()).is_err());
        assert!(!stage.path().join("mcw.exe").exists());
        assert!(!stage.path().join("mcw-tray.exe").exists());
        Ok(())
    }

    #[test]
    fn verified_truncated_zip_or_missing_exact_binary_fails_before_transaction() -> Result<()> {
        let complete = archive(&["mcw.exe", "mcw-tray.exe"])?;
        for bytes in [
            complete[..complete.len() / 2].to_vec(),
            archive(&["mcw.exe", "wrong-tray.exe"])?,
            archive(&["nested/mcw.exe", "mcw-tray.exe"])?,
        ] {
            let digest = digest(&bytes);
            let server = Server::new(bytes, format!("{digest}  {PACKAGE}"))?;
            let stage = tempfile::tempdir()?;
            assert!(stage_package(&server.release(Some(&digest), PACKAGE)?, stage.path()).is_err());
            assert_eq!(*server.requests.lock(), ["/sums", "/package"]);
        }
        Ok(())
    }
}
