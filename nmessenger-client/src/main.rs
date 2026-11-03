#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result {
    nmessenger_client::run_desktop()
}

#[cfg(target_os = "android")]
fn main() {}
