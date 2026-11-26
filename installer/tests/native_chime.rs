// Qualification driver, compiled by Cargo so its dependency metadata matches.
// This executable is never included in a distributed MicCamWatch package.
#[cfg(unix)]
fn main() {
    match miccamwatch::platform::play_chime() {
        Ok(()) => println!("native playback accepted"),
        Err(error) => {
            eprintln!("{error:#}");
            std::process::exit(2);
        }
    }
}

#[cfg(not(unix))]
fn main() {
    eprintln!("This qualification driver requires a Unix runner");
    std::process::exit(2);
}
