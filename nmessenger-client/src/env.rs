use std::path::Path;

/// Formats a raw host, IP, or URL string into a complete normalized HTTP/HTTPS server base URL.
///
/// Examples:
/// - `"192.168.1.16"` -> `"http://192.168.1.16:18087"`
/// - `"192.168.1.16:18087"` -> `"http://192.168.1.16:18087"`
/// - `"http://192.168.1.16:18087/"` -> `"http://192.168.1.16:18087"`
/// - `"https://example.com"` -> `"https://example.com"`
pub fn format_server_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches(['\'', '"']);
    if trimmed.is_empty() {
        return "https://nms.lawellx.com".to_string();
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
    let content = std::fs::read_to_string(path).ok()?;
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

/// Returns the default server URL for the client.
///
/// Priority:
/// 1. Runtime environment variables (`SERVER_URL`, `SERVER_IP`, `NM_SERVER_URL`, `API_URL`).
/// 2. Local `.env` file (at working directory or parent directory).
/// 3. Compile-time embedded URL (`CLIENT_DEFAULT_SERVER_URL`, configured at build time from `.env` or cargo env).
/// 4. Fallback default (`http://127.0.0.1:18087`).
pub fn default_server_url() -> String {
    let keys = ["SERVER_URL", "SERVER_IP", "NM_SERVER_URL", "NM_SERVER_IP", "API_URL"];
    for key in keys {
        if let Ok(val) = std::env::var(key) {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return format_server_url(trimmed);
            }
        }
    }

    if let Some(url) = parse_env_file(Path::new(".env")) {
        return url;
    }
    if let Some(url) = parse_env_file(Path::new("../.env")) {
        return url;
    }

    if let Some(compile_time) = option_env!("CLIENT_DEFAULT_SERVER_URL") {
        let trimmed = compile_time.trim();
        if !trimmed.is_empty() {
            return format_server_url(trimmed);
        }
    }

    "https://nms.lawellx.com".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_server_url_handles_all_variants() {
        assert_eq!(format_server_url(""), "https://nms.lawellx.com");
        assert_eq!(format_server_url("192.168.1.16"), "http://192.168.1.16:18087");
        assert_eq!(format_server_url("192.168.1.16:18087"), "http://192.168.1.16:18087");
        assert_eq!(format_server_url("http://192.168.1.16:18087/"), "http://192.168.1.16:18087");
        assert_eq!(format_server_url("https://example.com"), "https://example.com");
        assert_eq!(format_server_url("\"192.168.1.16:8080\""), "http://192.168.1.16:8080");
        assert_eq!(format_server_url("'http://10.0.0.2:18087'"), "http://10.0.0.2:18087");
    }
}

