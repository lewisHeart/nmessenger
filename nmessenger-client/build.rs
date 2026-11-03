use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn format_server_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches(['\'', '"']);
    if trimmed.is_empty() {
        return "http://127.0.0.1:18087".to_string();
    }
    let url = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else if trimmed.contains(':') {
        format!("http://{trimmed}")
    } else {
        format!("http://{trimmed}:18087")
    };
    url.trim_end_matches('/').to_string()
}

fn parse_env_file(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let keys = ["SERVER_URL", "SERVER_IP", "NM_SERVER_URL", "NM_SERVER_IP", "API_URL"];
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let key = k.trim();
            if keys.iter().any(|&target| target.eq_ignore_ascii_case(key)) {
                let val = v.trim();
                if !val.is_empty() {
                    return Some(format_server_url(val));
                }
            }
        }
    }
    None
}

fn main() {
    println!("cargo:rerun-if-changed=.env");
    println!("cargo:rerun-if-changed=../.env");
    println!("cargo:rerun-if-env-changed=SERVER_URL");
    println!("cargo:rerun-if-env-changed=SERVER_IP");
    println!("cargo:rerun-if-env-changed=NM_SERVER_URL");
    println!("cargo:rerun-if-env-changed=NM_SERVER_IP");
    println!("cargo:rerun-if-env-changed=API_URL");

    let keys = ["SERVER_URL", "SERVER_IP", "NM_SERVER_URL", "NM_SERVER_IP", "API_URL"];
    for key in keys {
        if let Ok(val) = env::var(key) {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                println!("cargo:rustc-env=CLIENT_DEFAULT_SERVER_URL={}", format_server_url(trimmed));
                return;
            }
        }
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into()));
    let candidates = [
        manifest_dir.join(".env"),
        manifest_dir.join("../.env"),
        PathBuf::from(".env"),
        PathBuf::from("../.env"),
    ];

    for candidate in candidates {
        if candidate.is_file() {
            if let Some(url) = parse_env_file(&candidate) {
                println!("cargo:rustc-env=CLIENT_DEFAULT_SERVER_URL={}", url);
                return;
            }
        }
    }

    println!("cargo:rustc-env=CLIENT_DEFAULT_SERVER_URL=http://127.0.0.1:18087");
}

