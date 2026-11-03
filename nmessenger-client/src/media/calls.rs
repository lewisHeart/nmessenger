use super::{MediaConfig, audio};
use crate::transport::{Api, Bus, Update};
use base64::{Engine, engine::general_purpose::STANDARD};
use nm_protocol::*;
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Component {
    Input,
    Output,
    Camera,
    Screen,
    RemoteAudio,
    RemoteVideo,
}

#[derive(Clone)]
pub enum CallEvent {
    Starting {
        activity: Uuid,
        epoch: u64,
    },
    Ready {
        activity: Uuid,
        epoch: u64,
        component: Component,
    },
    Failed {
        activity: Uuid,
        epoch: u64,
        component: Component,
        error: String,
    },
    ScreenSharing { activity: Uuid, epoch: u64, enabled: bool },
}

#[derive(Clone)]
pub struct CallStatus {
    pub activity: Uuid,
    pub epoch: u64,
    pub ready: HashSet<Component>,
    pub errors: BTreeMap<Component, String>,
    pub paused: bool,
    pub muted: bool,
    pub camera_enabled: bool,
    pub screen_sharing: bool,
}

struct Capture {
    stop: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
}
struct Call {
    id: Uuid,
    epoch: u64,
    stop: Arc<AtomicBool>,
    incoming: audio::Samples,
    sequences: HashMap<(Uuid, u8), u64>,
    announced: HashSet<u8>,
    local_user: Uuid,
    sharing_user: Option<Uuid>,
    remote_control: Arc<AtomicBool>,
    muted: Arc<AtomicBool>,
    camera_enabled: Arc<AtomicBool>,
    camera_stop: Arc<Mutex<Option<Arc<AtomicBool>>>>,
    screen: Arc<Mutex<Option<(Uuid, Arc<AtomicBool>)>>>,
    kind: ActivityKind,
}

#[derive(Default)]
pub struct MediaController {
    voice: Mutex<Option<Capture>>,
    call: Mutex<Option<Call>>,
}

impl Drop for MediaController {
    fn drop(&mut self) {
        self.stop_all();
    }
}

impl MediaController {
    pub fn stop_voice(&self, cancel: bool) {
        if let Some(capture) = self.voice.lock().unwrap().as_ref() {
            if cancel {
                capture.cancel.store(true, Ordering::Relaxed);
            }
            capture.stop.store(true, Ordering::Relaxed);
        }
    }

    pub fn stop_call(&self) {
        if let Some(call) = self.call.lock().unwrap().take() {
            call.stop.store(true, Ordering::Relaxed);
            if let Some(stop) = call.camera_stop.lock().unwrap().as_ref() {
                stop.store(true, Ordering::Relaxed);
            }
            if let Some((_, stop)) = call.screen.lock().unwrap().take() { stop.store(true, Ordering::Relaxed); }
        }
    }

    pub fn stop_all(&self) {
        self.stop_voice(true);
        self.stop_call();
        audio::stop_playback();
        super::playback::clear();
    }

    pub fn mute(&self, muted: bool) {
        if let Some(call) = self.call.lock().unwrap().as_ref() {
            call.muted.store(muted, Ordering::Relaxed);
        }
    }

    pub fn camera(&self, enabled: bool) {
        if let Some(call) = self.call.lock().unwrap().as_ref() {
            call.camera_enabled.store(enabled, Ordering::Relaxed);
            if !enabled && let Some(stop) = call.camera_stop.lock().unwrap().as_ref() {
                stop.store(true, Ordering::Relaxed);
            }
        }
    }

    pub fn screen(&self, enabled: bool, outgoing: Sender<ClientEvent>, bus: Bus) {
        let guard = self.call.lock().unwrap();
        let Some(call) = guard.as_ref().filter(|call| matches!(call.kind, ActivityKind::AudioCall | ActivityKind::VideoCall)) else { return; };
        let (id, epoch, local_user) = (call.id, call.epoch, call.local_user);
        let mut active = call.screen.lock().unwrap();
        if !enabled {
            if let Some((_, stop)) = active.take() { stop.store(true, Ordering::Relaxed); }
            bus.send(Update::CallMedia(CallEvent::ScreenSharing { activity: id, epoch, enabled: false }));
            let _ = outgoing.try_send(ClientEvent::Signal { activity_id: id, data: serde_json::json!({"screen_sharing": false}) });
            return;
        }
        if active.is_some() { return; }
        if !super::remote::available() {
            failure(&bus, id, epoch, Component::Screen, "Compartilhamento de tela indisponível nesta sessão.".into());
            bus.send(Update::CallMedia(CallEvent::ScreenSharing { activity: id, epoch, enabled: false }));
            return;
        }
        let token = Uuid::new_v4();
        let stop = Arc::new(AtomicBool::new(false));
        *active = Some((token, stop.clone()));
        let state = call.screen.clone();
        let call_stop = call.stop.clone();
        std::thread::spawn(move || {
            let observed = Arc::new(AtomicBool::new(false));
            let frame_bus = bus.clone();
            let frame_tx = outgoing.clone();
            let result = super::remote::capture(id, stop.clone(), outgoing.clone(), move |bytes| {
                if !observed.swap(true, Ordering::Relaxed) {
                    frame_bus.send(Update::CallMedia(CallEvent::ScreenSharing { activity: id, epoch, enabled: true }));
                    let _ = frame_tx.try_send(ClientEvent::Signal { activity_id: id, data: serde_json::json!({"screen_sharing": true}) });
                }
                if let Ok(mut frames) = super::images::decode(bytes) && !frames.is_empty() {
                    frame_bus.send(Update::LiveFrame { activity: id, epoch, from: local_user, stream: MediaStream::Screen, frame: frames.remove(0) });
                }
            });
            let owns_slot = {
                let mut active = state.lock().unwrap();
                if active.as_ref().is_some_and(|(current, _)| *current == token) { active.take(); true } else { false }
            };
            if owns_slot && !call_stop.load(Ordering::Relaxed) {
                if let Err(error) = result && !stop.load(Ordering::Relaxed) { failure(&bus, id, epoch, Component::Screen, error); }
                bus.send(Update::CallMedia(CallEvent::ScreenSharing { activity: id, epoch, enabled: false }));
                let _ = outgoing.try_send(ClientEvent::Signal { activity_id: id, data: serde_json::json!({"screen_sharing": false}) });
            }
        });
    }

    pub fn start_voice(&self, conversation: Uuid, input: String, api: Api, bus: Bus) {
        let epoch = super::next_sequence();
        let recording = |active| {
            bus.send(Update::Recording {
                epoch,
                conversation,
                active,
            })
        };
        if !audio::AVAILABLE {
            bus.send(Update::Error("Áudio nativo não habilitado.".into()));
            recording(false);
            return;
        }
        if self
            .call
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|call| !call.stop.load(Ordering::Relaxed))
        {
            bus.send(Update::Error(
                "Encerre a chamada antes de gravar uma mensagem de voz.".into(),
            ));
            recording(false);
            return;
        }
        let mut current = self.voice.lock().unwrap();
        if current
            .as_ref()
            .is_some_and(|capture| !capture.stop.load(Ordering::Relaxed))
        {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let cancel = Arc::new(AtomicBool::new(false));
        *current = Some(Capture {
            stop: stop.clone(),
            cancel: cancel.clone(),
        });
        recording(true);
        tokio::spawn(async move {
            let samples = tokio::task::spawn_blocking(move || {
                audio::record_on(&input, stop, true, |_| {}, || {})
            })
            .await;
            bus.send(Update::Recording {
                epoch,
                conversation,
                active: false,
            });
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let result = async {
                let samples = samples.map_err(|error| error.to_string())??;
                if samples.len() < 1600 {
                    return Err("Gravação muito curta (mínimo 0,1 segundo).".to_owned());
                }
                let duration = samples.len() as f32 / 16000.0;
                let file = api
                    .upload(
                        Some(conversation),
                        "mensagem-de-voz.wav".into(),
                        "audio/wav",
                        audio::wav(&samples)?,
                    )
                    .await?;
                let request = SendMessageRequest {
                    client_id: Uuid::new_v4(),
                    kind: MessageKind::Voice,
                    body: format!("Mensagem de voz · {duration:.1}s"),
                    style: MessageStyle::default(),
                    attachment_id: Some(file.id),
                };
                bus.send(Update::FileInfo(file));
                bus.send(Update::PreparedMessage {
                    conversation,
                    request,
                });
                Ok::<_, String>(())
            }
            .await;
            if let Err(error) = result {
                bus.send(Update::Error(error));
            }
        });
    }

    pub fn start_call(
        &self,
        activity: Activity,
        config: MediaConfig,
        local_user: Uuid,
        outgoing: Sender<ClientEvent>,
        bus: Bus,
        restart: bool,
    ) {
        if activity.status != ActivityStatus::Accepted
            || !matches!(
                activity.kind,
                ActivityKind::AudioCall | ActivityKind::VideoCall | ActivityKind::RemoteAssistance
            )
        {
            return;
        }
        let mut current = self.call.lock().unwrap();
        if current.as_ref().is_some_and(|call| call.id != activity.id && !call.stop.load(Ordering::Relaxed)) {
            bus.send(Update::Error(crate::i18n::tr(
                "End the current call before starting another.",
                "Encerre a chamada atual antes de iniciar outra.",
            ).into()));
            return;
        }
        if !restart
            && current
                .as_ref()
                .is_some_and(|call| call.id == activity.id && !call.stop.load(Ordering::Relaxed))
        {
            return;
        }
        if let Some(call) = current.take() {
            call.stop.store(true, Ordering::Relaxed);
            if let Some(stop) = call.camera_stop.lock().unwrap().as_ref() { stop.store(true, Ordering::Relaxed); }
            if let Some((_, stop)) = call.screen.lock().unwrap().take() { stop.store(true, Ordering::Relaxed); }
        }
        self.stop_voice(true);
        super::playback::stop();
        let epoch = super::next_sequence();
        let id = activity.id;
        let stop = Arc::new(AtomicBool::new(false));
        let muted = Arc::new(AtomicBool::new(config.muted));
        let camera_enabled = Arc::new(AtomicBool::new(config.camera_enabled && (restart || activity.kind == ActivityKind::VideoCall)));
        let camera_stop = Arc::new(Mutex::new(None));
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        *current = Some(Call {
            id,
            epoch,
            stop: stop.clone(),
            incoming: queue.clone(),
            sequences: HashMap::new(),
            announced: HashSet::new(),
            local_user,
            sharing_user: (activity.kind == ActivityKind::RemoteAssistance)
                .then_some(activity.initiator_id),
            remote_control: Arc::new(AtomicBool::new(false)),
            muted: muted.clone(),
            camera_enabled: camera_enabled.clone(),
            camera_stop: camera_stop.clone(),
            screen: Arc::new(Mutex::new(None)),
            kind: activity.kind,
        });
        bus.send(Update::CallMedia(CallEvent::Starting {
            activity: id,
            epoch,
        }));
        if activity.kind == ActivityKind::RemoteAssistance {
            if activity.initiator_id == local_user {
                std::thread::spawn(move || {
                    let observed = Arc::new(AtomicBool::new(false));
                    let ready_bus = bus.clone();
                    if let Err(error) = super::remote::capture(id, stop, outgoing, move |_| {
                        if !observed.swap(true, Ordering::Relaxed) {
                            ready(&ready_bus, id, epoch, Component::Camera);
                        }
                    }) {
                        failure(&bus, id, epoch, Component::Camera, error);
                    }
                });
            }
            return;
        }
        let output_stop = stop.clone();
        let output_bus = bus.clone();
        std::thread::spawn(move || {
            for attempt in 0..3 {
                if output_stop.load(Ordering::Relaxed) {
                    return;
                }
                let ready_bus = output_bus.clone();
                match audio::output_on(
                    &config.output,
                    queue.clone(),
                    output_stop.clone(),
                    Arc::new(AtomicBool::new(false)),
                    move || ready(&ready_bus, id, epoch, Component::Output),
                ) {
                    Ok(()) => return,
                    Err(_) if attempt < 2 => {
                        std::thread::sleep(std::time::Duration::from_millis(120))
                    }
                    Err(error) => {
                        failure(&output_bus, id, epoch, Component::Output, error);
                        return;
                    }
                }
            }
        });
        let input_stop = stop.clone();
        let input_bus = bus.clone();
        let input_tx = outgoing.clone();
        std::thread::spawn(move || {
            for attempt in 0..3 {
                if input_stop.load(Ordering::Relaxed) {
                    return;
                }
                let tx = input_tx.clone();
                let muted = muted.clone();
                let ready_bus = input_bus.clone();
                let result = audio::record_on(
                    &config.input,
                    input_stop.clone(),
                    false,
                    move |mut samples| {
                        if muted.load(Ordering::Relaxed) {
                            samples.fill(0);
                        }
                        let bytes: Vec<u8> = samples
                            .iter()
                            .flat_map(|sample| sample.to_le_bytes())
                            .collect();
                        let _ = tx.try_send(ClientEvent::Media {
                            activity_id: id,
                            stream: MediaStream::Audio,
                            sequence: super::next_sequence(),
                            data: STANDARD.encode(bytes),
                        });
                    },
                    move || ready(&ready_bus, id, epoch, Component::Input),
                );
                match result {
                    Ok(_) => return,
                    Err(_) if attempt < 2 => {
                        std::thread::sleep(std::time::Duration::from_millis(120))
                    }
                    Err(error) => {
                        failure(&input_bus, id, epoch, Component::Input, error);
                        return;
                    }
                }
            }
        });
        if matches!(activity.kind, ActivityKind::AudioCall | ActivityKind::VideoCall) {
            std::thread::spawn(move || {
                if restart {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                while !stop.load(Ordering::Relaxed) {
                    if !camera_enabled.load(Ordering::Relaxed) {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        continue;
                    }
                    let capture_stop = Arc::new(AtomicBool::new(false));
                    {
                        let mut active = camera_stop.lock().unwrap();
                        if stop.load(Ordering::Relaxed) || !camera_enabled.load(Ordering::Relaxed) { continue; }
                        *active = Some(capture_stop.clone());
                    }
                    let observed = Arc::new(AtomicBool::new(false));
                    let observer_bus = bus.clone();
                    let spec = super::video::CaptureSpec {
                        activity: id, stream: MediaStream::Camera,
                        input: super::video::camera_input(&config.camera), width: 640, fps: 12,
                    };
                    let result = super::video::capture(spec, capture_stop.clone(), outgoing.clone(), move |bytes| {
                        if !observed.swap(true, Ordering::Relaxed) { ready(&observer_bus, id, epoch, Component::Camera); }
                        if let Ok(mut frames) = super::images::decode(bytes) && !frames.is_empty() {
                            observer_bus.send(Update::LiveFrame { activity: id, epoch, from: local_user, stream: MediaStream::Camera, frame: frames.remove(0) });
                        }
                    });
                    *camera_stop.lock().unwrap() = None;
                    if let Err(error) = result && !capture_stop.load(Ordering::Relaxed) && !stop.load(Ordering::Relaxed) {
                        failure(&bus, id, epoch, Component::Camera, error);
                        camera_enabled.store(false, Ordering::Relaxed);
                    }
                }
            });
        }
    }

    pub fn receive(
        &self,
        id: Uuid,
        from: Uuid,
        stream: MediaStream,
        sequence: u64,
        data: String,
        bus: Bus,
    ) {
        if data.len() > MAX_MEDIA_FRAME_BYTES * 4 / 3 + 4 {
            return;
        }
        let mut guard = self.call.lock().unwrap();
        let Some(call) = guard
            .as_mut()
            .filter(|call| call.id == id && !call.stop.load(Ordering::Relaxed))
        else {
            return;
        };
        if call
            .sharing_user
            .is_some_and(|owner| owner != from || stream != MediaStream::Screen)
        {
            return;
        }
        let stream_key = match stream {
            MediaStream::Audio => 0,
            MediaStream::Camera => 1,
            MediaStream::Screen => 2,
        };
        let key = (from, stream_key);
        if call
            .sequences
            .get(&key)
            .is_some_and(|last| *last >= sequence)
        {
            return;
        }
        let Ok(bytes) = STANDARD.decode(data) else {
            return;
        };
        call.sequences.insert(key, sequence);
        let epoch = call.epoch;
        match stream {
            MediaStream::Audio => {
                let mut queue = call.incoming.lock().unwrap();
                while queue.len() > 8000 {
                    queue.pop_front();
                }
                queue.extend(
                    bytes
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|bytes| i16::from_le_bytes(*bytes)),
                );
                if call.announced.insert(0) {
                    ready(&bus, id, epoch, Component::RemoteAudio);
                }
            }
            MediaStream::Camera | MediaStream::Screen => {
                static FRAME_DECODERS: tokio::sync::Semaphore =
                    tokio::sync::Semaphore::const_new(2);
                let Ok(permit) = FRAME_DECODERS.try_acquire() else {
                    return;
                };
                if call.announced.insert(stream_key) {
                    ready(&bus, id, epoch, Component::RemoteVideo);
                }
                tokio::task::spawn_blocking(move || {
                    let _permit = permit;
                    if let Ok(mut frames) = super::images::decode(&bytes)
                        && !frames.is_empty()
                    {
                        bus.send(Update::LiveFrame {
                            activity: id,
                            epoch,
                            from,
                            stream,
                            frame: frames.remove(0),
                        });
                    }
                });
            }
        }
    }

    pub fn remote_control(&self, activity: Uuid, enabled: bool) {
        if let Some(call) = self.call.lock().unwrap().as_ref()
            && call.id == activity
            && call.sharing_user == Some(call.local_user)
        {
            call.remote_control.store(enabled, Ordering::Relaxed);
        }
    }

    pub fn remote_input(&self, activity: Uuid, from: Uuid, data: serde_json::Value, bus: Bus) {
        let guard = self.call.lock().unwrap();
        let Some(call) = guard.as_ref().filter(|call| {
            call.id == activity
                && call.sharing_user == Some(call.local_user)
                && from != call.local_user
                && call.remote_control.load(Ordering::Relaxed)
                && !call.stop.load(Ordering::Relaxed)
        }) else {
            return;
        };
        let Ok(input) = serde_json::from_value::<super::remote::Input>(data) else {
            return;
        };
        let permission = call.remote_control.clone();
        let stop = call.stop.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(error) = super::remote::execute(input, permission, stop) {
                bus.send(Update::Error(error));
            }
        });
    }
}

fn ready(bus: &Bus, activity: Uuid, epoch: u64, component: Component) {
    bus.send(Update::CallMedia(CallEvent::Ready {
        activity,
        epoch,
        component,
    }));
}
fn failure(bus: &Bus, activity: Uuid, epoch: u64, component: Component, error: String) {
    bus.send(Update::CallMedia(CallEvent::Failed {
        activity,
        epoch,
        component,
        error,
    }));
}

#[cfg(test)]
mod tests;
