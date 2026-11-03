//! Rich "now playing" presence from the desktop, with manual override.
//!
//! On Linux the track is read from any MPRIS player (Spotify, Rhythmbox,
//! browsers…). The manual value in the profile always wins while it is set.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::Receiver,
};

pub struct Watcher {
    receiver: Receiver<Option<String>>,
    stop: Arc<AtomicBool>,
}

impl Watcher {
    /// Returns `Some` only when the detected track changed.
    pub fn changed(&self) -> Option<Option<String>> {
        self.receiver.try_recv().ok()
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Automatic presence must never overwrite something the user typed or set on another client.
pub fn should_publish(
    enabled: bool,
    current: &str,
    published: Option<&str>,
    track: Option<&str>,
) -> Option<String> {
    if !enabled {
        if !current.is_empty() && published == Some(current) {
            return Some(String::new());
        }
        return None;
    }
    // If the song was not published by this watcher, consider it manual or from another client.
    let manual = !current.is_empty() && published != Some(current);
    if manual {
        return None;
    }
    // If this watcher published the track, but no track is playing anymore, clear it.
    if track.is_none() {
        if !current.is_empty() && published == Some(current) {
            return Some(String::new());
        }
        return None;
    }
    let desired = track.unwrap_or("");
    (desired != current).then(|| desired.to_owned())
}

#[cfg(target_os = "linux")]
pub fn watch() -> Option<Watcher> {
    use std::{sync::mpsc, time::Duration};
    let (sender, receiver) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = stop.clone();
    std::thread::Builder::new()
        .name("nm-now-playing".into())
        .spawn(move || {
            let mut last: Option<Option<String>> = None;
            while !thread_stop.load(Ordering::Relaxed) {
                let value = read_linux();
                if last.as_ref() != Some(&value) {
                    if sender.send(value.clone()).is_err() {
                        return;
                    }
                    last = Some(value);
                }
                for _ in 0..15 {
                    if thread_stop.load(Ordering::Relaxed) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        })
        .ok()?;
    Some(Watcher { receiver, stop })
}

#[cfg(not(target_os = "linux"))]
pub fn watch() -> Option<Watcher> {
    None
}

#[cfg(target_os = "linux")]
fn read_linux() -> Option<String> {
    zbus::block_on(async {
        use std::collections::HashMap;
        use zbus::{Connection, OwnedVariant, Proxy, fdo::DBusProxy};
        let connection = Connection::session().await.ok()?;
        let bus = DBusProxy::new(&connection).await.ok()?;
        let mut names = bus.list_names().await.ok()?;
        names.sort_by_key(|name| {
            (
                !name.as_str().starts_with("org.mpris.MediaPlayer2.spotify"),
                name.to_string(),
            )
        });
        for name in names {
            let name = name.as_str();
            if !name.starts_with("org.mpris.MediaPlayer2.") {
                continue;
            }
            let Ok(player) = Proxy::new(
                &connection,
                name,
                "/org/mpris/MediaPlayer2",
                "org.mpris.MediaPlayer2.Player",
            )
            .await
            else {
                continue;
            };
            let Ok(status) = player.get_property::<String>("PlaybackStatus").await else {
                continue;
            };
            if status != "Playing" {
                continue;
            }
            let Ok(metadata) = player.get_property::<OwnedVariant>("Metadata").await else {
                continue;
            };
            let Ok(mut fields) = <HashMap<String, OwnedVariant>>::try_from(metadata) else {
                continue;
            };
            let title = fields
                .remove("xesam:title")
                .and_then(|value| String::try_from(value).ok());
            let artists = fields
                .remove("xesam:artist")
                .and_then(|value| Vec::<String>::try_from(value).ok())
                .unwrap_or_default();
            let artist = artists.first().cloned();
            let Some(title) = title.filter(|title| !title.trim().is_empty()) else {
                continue;
            };
            let player_name = name
                .strip_prefix("org.mpris.MediaPlayer2.")
                .filter(|value| !value.is_empty());
            let label = match (artist, player_name) {
                (Some(artist), _) if !artist.is_empty() => format!("{artist} — {title}"),
                (_, Some(player)) => format!("{title} · {}", capitalize(player)),
                _ => title,
            };
            return Some(
                label
                    .chars()
                    .filter(|ch| !ch.is_control())
                    .take(256)
                    .collect(),
            );
        }
        None
    })
}

#[cfg(target_os = "linux")]
fn capitalize(value: &str) -> String {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::should_publish;

    #[test]
    fn manual_music_is_kept_and_automatic_tracks_publish_only_on_change() {
        assert_eq!(
            should_publish(true, "", None, Some("A — B")),
            Some("A — B".into())
        );
        assert_eq!(
            should_publish(true, "A — B", Some("A — B"), Some("A — B")),
            None
        );
        assert_eq!(
            should_publish(true, "A — B", Some("A — B"), None),
            Some(String::new())
        );
        assert_eq!(
            should_publish(true, "Minha música", Some("A — B"), Some("C — D")),
            None
        );
        assert_eq!(should_publish(false, "", None, Some("A — B")), None);
        assert_eq!(
            should_publish(false, "A — B", Some("A — B"), Some("A — B")),
            Some(String::new())
        );
        assert_eq!(should_publish(false, "Manual", None, Some("A — B")), None);
        assert_eq!(
            should_publish(true, "Minha música", None, None),
            None
        );
        assert_eq!(
            should_publish(true, "Música do PC", None, None),
            None
        );
    }

    #[cfg(target_os = "linux")]
    struct Player {
        playing: bool,
    }

    #[cfg(target_os = "linux")]
    #[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
    impl Player {
        #[zbus(property)]
        fn playback_status(&self) -> &str {
            if self.playing { "Playing" } else { "Paused" }
        }
        #[zbus(property)]
        fn metadata(&self) -> std::collections::HashMap<String, zbus::Variant<'_>> {
            std::collections::HashMap::from([
                (
                    "xesam:title".into(),
                    zbus::Variant::from("Canção de teste"),
                ),
                (
                    "xesam:artist".into(),
                    zbus::Variant::from(vec!["Artista"]),
                ),
            ])
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    #[ignore = "run inside an isolated dbus-run-session with NM_TEST_MPRIS=1"]
    async fn mpris_reads_real_dbus_properties_and_clears_paused_music() {
        assert_eq!(std::env::var("NM_TEST_MPRIS").as_deref(), Ok("1"));
        let connection = zbus::connection::Builder::session()
            .name("org.mpris.MediaPlayer2.spotify.nmessenger_test")
            .serve_at("/org/mpris/MediaPlayer2", Player { playing: true })
            .build()
            .await
            .unwrap();
        assert_eq!(
            tokio::task::spawn_blocking(super::read_linux)
                .await
                .unwrap(),
            Some("Artista — Canção de teste".into())
        );
        let player = connection
            .object_server()
            .interface::<_, Player>("/org/mpris/MediaPlayer2")
            .await
            .unwrap();
        player.get_mut().await.playing = false;
        assert_eq!(
            tokio::task::spawn_blocking(super::read_linux)
                .await
                .unwrap(),
            None
        );
    }
}
