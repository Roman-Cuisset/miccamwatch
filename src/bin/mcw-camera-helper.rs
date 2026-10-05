#[cfg(target_os = "linux")]
fn main() {
    if let Err(error) = miccamwatch::privacy::run_helper() {
        eprintln!("mcw-camera-helper: {error:#}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("mcw-camera-helper is supported on Linux only");
    std::process::exit(1);
}
