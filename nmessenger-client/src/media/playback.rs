use super::audio::{self, Samples};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use uuid::Uuid;

#[derive(Clone)]
pub struct Snapshot {
    pub duration: f32,
    pub position: f32,
    pub playing: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub speed: f32,
}
impl Default for Snapshot {
    fn default() -> Self { Self { duration: 0.0, position: 0.0, playing: false, loading: false, error: None, speed: 1.0 } }
}
struct Clip {
    samples: Arc<Vec<i16>>,
    error: Option<String>,
    loading: bool,
    updated: std::time::Instant,
    speed: f32,
}
struct Active {
    id: Uuid,
    queue: Samples,
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
#[derive(Default)]
struct Player {
    clips: HashMap<Uuid, Clip>,
    active: Option<Active>,
}
static PLAYER: Mutex<Option<Player>> = Mutex::new(None);
#[cfg(test)]
pub(crate) static TEST_LOCK: Mutex<()> = Mutex::new(());

pub fn begin_load(id: Uuid) -> bool {
    let mut guard = PLAYER.lock().unwrap();
    let player = guard.get_or_insert_with(Player::default);
    if player
        .clips
        .get(&id)
        .is_some_and(|clip| clip.error.is_none())
    {
        return false;
    }
    player.clips.insert(
        id,
        Clip {
            samples: Arc::new(Vec::new()),
            error: None,
            loading: true,
            updated: std::time::Instant::now(),
            speed: 1.0,
        },
    );
    true
}

pub fn loaded(id: Uuid, result: Result<Vec<i16>, String>) {
    let mut guard = PLAYER.lock().unwrap();
    let player = guard.get_or_insert_with(Player::default);
    let speed = player.clips.get(&id).map_or(1.0, |clip| clip.speed);
    let (samples, error) = match result {
        Ok(samples) => (samples, None),
        Err(error) => (Vec::new(), Some(error)),
    };
    player.clips.insert(
        id,
        Clip {
            samples: Arc::new(samples),
            error,
            loading: false,
            updated: std::time::Instant::now(),
            speed,
        },
    );
    while player.clips.len() > 20 {
        let active = player.active.as_ref().map(|session| session.id);
        let oldest = player
            .clips
            .iter()
            .filter(|(key, clip)| **key != id && Some(**key) != active && !clip.loading)
            .min_by_key(|(_, clip)| clip.updated)
            .map(|(key, _)| *key);
        if let Some(key) = oldest {
            player.clips.remove(&key);
        } else {
            break;
        }
    }
}

pub fn snapshot(id: Uuid) -> Snapshot {
    let guard = PLAYER.lock().unwrap();
    let Some(player) = guard.as_ref() else {
        return Snapshot::default();
    };
    let Some(clip) = player.clips.get(&id) else {
        return Snapshot::default();
    };
    let mut result = Snapshot {
        duration: clip.samples.len() as f32 / 16000.0,
        loading: clip.loading,
        error: clip.error.clone(),
        speed: clip.speed,
        ..Default::default()
    };
    if let Some(active) = player.active.as_ref().filter(|active| active.id == id) {
        let remaining = active.queue.lock().unwrap().len();
        result.position = if remaining == 0 {
            0.0
        } else {
            (clip.samples.len() as f32 - remaining as f32 * clip.speed).max(0.0) / 16000.0
        };
        result.playing = remaining > 0
            && active.ready.load(Ordering::Relaxed)
            && !active.paused.load(Ordering::Relaxed)
            && !active.stop.load(Ordering::Relaxed);
        result.loading |=
            !active.ready.load(Ordering::Relaxed) && !active.stop.load(Ordering::Relaxed);
    }
    result
}

pub fn toggle(id: Uuid, output: String) -> Result<(), String> {
    let mut guard = PLAYER.lock().unwrap();
    let player = guard.get_or_insert_with(Player::default);
    let (samples, speed) = player
        .clips
        .get(&id)
        .filter(|clip| !clip.loading && !clip.samples.is_empty())
        .map(|clip| (clip.samples.clone(), clip.speed))
        .ok_or("O áudio ainda não foi carregado.")?;
    if let Some(clip) = player.clips.get_mut(&id) {
        clip.error = None;
    }
    if let Some(active) = player
        .active
        .as_mut()
        .filter(|active| active.id == id && !active.stop.load(Ordering::Relaxed))
    {
        let mut queue = active.queue.lock().unwrap();
        if queue.is_empty() {
            queue.extend(at_speed(&samples, speed));
            active.paused.store(false, Ordering::Relaxed);
        } else {
            active.paused.fetch_xor(true, Ordering::Relaxed);
        }
        return Ok(());
    }
    let previous = player.active.take().and_then(|mut active| {
        active.stop.store(true, Ordering::Relaxed);
        active.thread.take()
    });
    let queue = Arc::new(Mutex::new(at_speed(&samples, speed)));
    let stop = Arc::new(AtomicBool::new(false));
    let paused = Arc::new(AtomicBool::new(false));
    let ready = Arc::new(AtomicBool::new(false));
    let worker_queue = queue.clone();
    let worker_stop = stop.clone();
    let worker_paused = paused.clone();
    let worker_ready = ready.clone();
    let thread = std::thread::spawn(move || {
        if let Some(previous) = previous {
            let _ = previous.join();
        }
        if let Err(error) = audio::output_on(
            &output,
            worker_queue,
            worker_stop.clone(),
            worker_paused,
            move || {
                worker_ready.store(true, Ordering::Relaxed);
                if let Some(player) = PLAYER.lock().unwrap().as_mut()
                    && let Some(clip) = player.clips.get_mut(&id)
                {
                    clip.error = None;
                }
            },
        ) {
            worker_stop.store(true, Ordering::Relaxed);
            if let Some(player) = PLAYER.lock().unwrap().as_mut()
                && let Some(clip) = player.clips.get_mut(&id)
            {
                clip.error = Some(error);
            }
        }
    });
    player.active = Some(Active {
        id,
        queue,
        stop,
        paused,
        ready,
        thread: Some(thread),
    });
    Ok(())
}

pub fn seek(id: Uuid, seconds: f32) {
    let mut guard = PLAYER.lock().unwrap();
    let Some(player) = guard.as_mut() else {
        return;
    };
    if let Some((samples, speed)) = player.clips.get(&id).map(|clip| (clip.samples.clone(), clip.speed))
        && let Some(active) = player.active.as_mut().filter(|a| a.id == id)
    {
        let position = (seconds.max(0.0) * 16000.0) as usize;
        *active.queue.lock().unwrap() = at_speed(&samples[position.min(samples.len())..], speed);
    }
}

fn at_speed(samples: &[i16], speed: f32) -> VecDeque<i16> {
    let mut output = VecDeque::with_capacity((samples.len() as f32 / speed).ceil() as usize);
    let mut position = 0.0_f64;
    while (position as usize) < samples.len() {
        let index = position as usize;
        let next = (index + 1).min(samples.len() - 1);
        let fraction = (position - index as f64) as f32;
        output.push_back((samples[index] as f32 * (1.0 - fraction) + samples[next] as f32 * fraction).round() as i16);
        position += speed as f64;
    }
    output
}

pub fn set_speed(id: Uuid, speed: f32) {
    if ![1.0_f32, 1.2, 1.5, 2.0].contains(&speed) { return; }
    let mut guard = PLAYER.lock().unwrap();
    let Some(player) = guard.as_mut() else { return; };
    let Some(clip) = player.clips.get_mut(&id) else { return; };
    if let Some(active) = player.active.as_mut().filter(|active| active.id == id) {
        let mut queue = active.queue.lock().unwrap();
        let remaining = (queue.len() as f32 * clip.speed).round() as usize;
        let position = clip.samples.len().saturating_sub(remaining);
        *queue = at_speed(&clip.samples[position..], speed);
    }
    clip.speed = speed;
}

pub fn next_speed(current: f32) -> f32 {
    let speeds = [1.0_f32, 1.2, 1.5, 2.0];
    let index = speeds.iter().position(|value| (current - value).abs() < 0.01).unwrap_or(0);
    speeds[(index + 1) % speeds.len()]
}

pub fn clear() {
    if let Some(mut player) = PLAYER.lock().unwrap().take()
        && let Some(active) = player.active.take()
    {
        active.stop.store(true, Ordering::Relaxed);
    }
}

pub fn stop() {
    #[cfg(test)]
    let _test_lock = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    if let Some(player) = PLAYER.lock().unwrap().as_mut()
        && let Some(active) = player.active.as_mut()
    {
        active.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn playback_timeline_supports_pause_resume_seek_and_replay_without_devices() {
        let _lock = TEST_LOCK.lock().unwrap();
        let id = Uuid::new_v4();
        loaded(id, Ok(vec![2000; 32000]));
        let queue = Arc::new(Mutex::new(VecDeque::from(vec![2000; 32000])));
        {
            let mut guard = PLAYER.lock().unwrap();
            let player = guard.as_mut().unwrap();
            player.active = Some(Active {
                id,
                queue: queue.clone(),
                stop: Arc::new(AtomicBool::new(false)),
                paused: Arc::new(AtomicBool::new(false)),
                ready: Arc::new(AtomicBool::new(true)),
                thread: None,
            });
        }
        assert_eq!(snapshot(id).duration, 2.0);
        queue.lock().unwrap().drain(..16000);
        assert_eq!(snapshot(id).position, 1.0);
        toggle(id, String::new()).unwrap();
        assert!(!snapshot(id).playing);
        set_speed(id, 2.0);
        assert_eq!(snapshot(id).speed, 2.0);
        assert_eq!(snapshot(id).position, 1.0);
        assert_eq!(queue.lock().unwrap().len(), 8000);
        seek(id, 1.5);
        assert_eq!(snapshot(id).position, 1.5);
        assert_eq!(queue.lock().unwrap().len(), 4000);
        set_speed(id, 1.2);
        assert!((snapshot(id).position - 1.5).abs() < 0.001);
        set_speed(id, 4.0);
        assert_eq!(snapshot(id).speed, 1.2);
        toggle(id, String::new()).unwrap();
        assert!(snapshot(id).playing);
        seek(id, 10.0);
        assert!(!snapshot(id).playing);
        toggle(id, String::new()).unwrap();
        assert!(snapshot(id).playing);
        assert_eq!(snapshot(id).position, 0.0);
        clear();
    }

    #[test]
    fn playback_speed_cycles_in_order() {
        assert_eq!(next_speed(1.0), 1.2);
        assert_eq!(next_speed(1.2), 1.5);
        assert_eq!(next_speed(1.5), 2.0);
        assert_eq!(next_speed(2.0), 1.0);
    }
}
