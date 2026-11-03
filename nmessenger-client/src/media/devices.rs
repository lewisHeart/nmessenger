#[derive(Clone, Default)]
pub struct DeviceCatalog {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub cameras: Vec<String>,
    pub ffmpeg: bool,
    pub error: Option<String>,
}

pub fn discover() -> DeviceCatalog {
    if super::test_media() {
        return DeviceCatalog {
            inputs: vec!["test:sine".into()],
            outputs: vec!["test:null".into()],
            cameras: vec!["test:pattern".into()],
            ffmpeg: true,
            error: None,
        };
    }
    let mut result = DeviceCatalog::default();
    #[cfg(feature = "native-audio")]
    {
        use cpal::traits::{DeviceTrait, HostTrait};
        let host = cpal::default_host();
        match host.input_devices() {
            Ok(devices) => {
                result.inputs = devices
                    .filter_map(|device| {
                        device
                            .description()
                            .ok()
                            .map(|description| description.name().to_owned())
                    })
                    .collect()
            }
            Err(error) => result.error = Some(format!("Microfones: {error}")),
        }
        match host.output_devices() {
            Ok(devices) => {
                result.outputs = devices
                    .filter_map(|device| {
                        device
                            .description()
                            .ok()
                            .map(|description| description.name().to_owned())
                    })
                    .collect()
            }
            Err(error) => result.error = Some(format!("Saídas de áudio: {error}")),
        }
    }
    #[cfg(target_os = "linux")]
    if let Ok(entries) = std::fs::read_dir("/dev") {
        result.cameras = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                (name.starts_with("video") && name[5..].chars().all(|c| c.is_ascii_digit()))
                    .then(|| entry.path().to_string_lossy().into_owned())
            })
            .collect();
    }
    #[cfg(target_os = "windows")]
    { result.cameras = super::video::windows_cameras(); }
    result.inputs.sort_by(|a, b| {
        let a_server = a.contains("PulseAudio") || a.contains("PipeWire");
        let b_server = b.contains("PulseAudio") || b.contains("PipeWire");
        let a_discard = a.starts_with("Discard");
        let b_discard = b.starts_with("Discard");
        match (a_server, b_server, a_discard, b_discard) {
            (true, false, _, _) => std::cmp::Ordering::Less,
            (false, true, _, _) => std::cmp::Ordering::Greater,
            (_, _, true, false) => std::cmp::Ordering::Greater,
            (_, _, false, true) => std::cmp::Ordering::Less,
            _ => a.cmp(b),
        }
    });
    result.inputs.dedup();
    result.outputs.sort_by(|a, b| {
        let a_server = a.contains("PulseAudio") || a.contains("PipeWire");
        let b_server = b.contains("PulseAudio") || b.contains("PipeWire");
        let a_discard = a.starts_with("Discard");
        let b_discard = b.starts_with("Discard");
        match (a_server, b_server, a_discard, b_discard) {
            (true, false, _, _) => std::cmp::Ordering::Less,
            (false, true, _, _) => std::cmp::Ordering::Greater,
            (_, _, true, false) => std::cmp::Ordering::Greater,
            (_, _, false, true) => std::cmp::Ordering::Less,
            _ => a.cmp(b),
        }
    });
    result.outputs.dedup();
    result.cameras.sort();
    result.ffmpeg = std::process::Command::new("ffmpeg")
        .args(["-version"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    result
}

#[cfg(feature = "native-audio")]
pub fn input(name: &str) -> Result<cpal::Device, String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    let devices: Vec<_> = host.input_devices().map_err(|e| e.to_string())?.collect();

    // 1. If user selected a specific device, try it first
    if !name.is_empty()
        && let Some(dev) = devices
            .iter()
            .find(|d| d.description().is_ok_and(|desc| desc.name() == name))
    {
        return Ok(dev.clone());
    }

    // 2. On Linux, sound servers (PulseAudio / PipeWire / default) must be prioritized over raw ALSA
    // because PipeWire holds the hardware ALSA sink, making direct ALSA access fail with "Device or resource busy".
    #[cfg(target_os = "linux")]
    {
        for keyword in ["PulseAudio", "PipeWire", "pulse", "default"] {
            if let Some(dev) = devices.iter().find(|d| {
                d.description()
                    .is_ok_and(|desc| desc.name().contains(keyword))
            }) {
                return Ok(dev.clone());
            }
        }
    }

    // 3. Fallback to host default device
    if let Some(dev) = host.default_input_device() {
        return Ok(dev);
    }

    // 4. Fallback to first available non-discard device
    if let Some(dev) = devices.iter().find(|d| {
        d.description()
            .is_ok_and(|desc| !desc.name().starts_with("Discard"))
    }) {
        return Ok(dev.clone());
    }

    devices.into_iter().next().ok_or_else(|| {
        "Nenhum microfone encontrado. Selecione um dispositivo em Configurações > Áudio e vídeo.".into()
    })
}

#[cfg(feature = "native-audio")]
pub fn output(name: &str) -> Result<cpal::Device, String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    let devices: Vec<_> = host.output_devices().map_err(|e| e.to_string())?.collect();

    // 1. If user selected a specific device, try it first
    if !name.is_empty()
        && let Some(dev) = devices
            .iter()
            .find(|d| d.description().is_ok_and(|desc| desc.name() == name))
    {
        return Ok(dev.clone());
    }

    // 2. On Linux, sound servers (PulseAudio / PipeWire / default) must be prioritized over raw ALSA
    // because PipeWire holds the hardware ALSA sink, making direct ALSA access fail with "Device or resource busy".
    #[cfg(target_os = "linux")]
    {
        for keyword in ["PulseAudio", "PipeWire", "pulse", "default"] {
            if let Some(dev) = devices.iter().find(|d| {
                d.description()
                    .is_ok_and(|desc| desc.name().contains(keyword))
            }) {
                return Ok(dev.clone());
            }
        }
    }

    // 3. Fallback to host default device
    if let Some(dev) = host.default_output_device() {
        return Ok(dev);
    }

    // 4. Fallback to first available non-discard device
    if let Some(dev) = devices.iter().find(|d| {
        d.description()
            .is_ok_and(|desc| !desc.name().starts_with("Discard"))
    }) {
        return Ok(dev.clone());
    }

    devices.into_iter().next().ok_or_else(|| {
        "Nenhuma saída de áudio encontrada. Selecione um dispositivo em Configurações > Áudio e vídeo.".into()
    })
}
