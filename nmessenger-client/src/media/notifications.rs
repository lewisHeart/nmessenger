use std::f32::consts::TAU;

/// Windows Live Messenger style nudge: dual mechanical rattle/strike with bass shudder and metallic snap.
fn nudge_pcm() -> Vec<i16> {
    let sample_rate = 16_000.0_f32;
    let total_samples = 14_080; // 0.88s @ 16 kHz
    let mut samples = Vec::with_capacity(total_samples);
    let mut phase1 = 0.0_f32;
    let mut phase2 = 0.0_f32;

    for i in 0..total_samples {
        let t = i as f32 / sample_rate;
        let mut sample = 0.0_f32;

        // Strike 1 (0.0s to 0.18s): initial window shudder
        if t < 0.18 {
            let env = (1.0 - t / 0.18).powf(1.4);
            let attack = (t / 0.005).min(1.0);
            let freq = 155.0 - (t / 0.18) * 85.0; // 155Hz -> 70Hz
            phase1 += TAU * freq / sample_rate;
            let tremor = (TAU * 45.0 * t).sin() * 0.42 + 0.58;
            let bass = phase1.sin() * tremor;
            let snap = (TAU * 920.0 * t).sin() * (-t * 110.0).exp();
            sample += (bass * 0.75 + snap * 0.35) * env * attack * 18_000.0;
        }

        // Strike 2 (0.22s to 0.88s): signature loud WLM impact with whip-crack & ringing reverberation
        if t >= 0.22 {
            let t2 = t - 0.22;
            let dur = 0.66;
            let env = (1.0 - t2 / dur).max(0.0).powf(1.6);
            let attack = (t2 / 0.006).min(1.0);
            let freq = 175.0 - (t2 / dur).min(1.0) * 120.0; // 175Hz -> 55Hz
            phase2 += TAU * freq / sample_rate;
            let tremor = (TAU * 42.0 * t2).sin() * 0.45 + 0.55;
            let bass = phase2.sin() * tremor;
            // High metallic whip/crack transient
            let crack = (TAU * 1160.0 * t2).sin() * (-t2 * 85.0).exp()
                + (TAU * 1740.0 * t2).sin() * 0.45 * (-t2 * 110.0).exp();
            // Harmonic metallic chime ringing body
            let chime = (TAU * 520.0 * t2).sin() * 0.28 * (-t2 * 7.5).exp()
                + (TAU * 830.0 * t2).sin() * 0.16 * (-t2 * 11.0).exp();
            sample += (bass * 0.65 + crack * 0.48 + chime * 0.30) * env * attack * 23_500.0;
        }

        samples.push(sample.clamp(-32000.0, 32000.0) as i16);
    }
    samples
}

/// Native fallback tones: newly composed cues, not recovered Messenger audio.
pub fn tone(event: &str) -> Result<Vec<u8>, String> {
    if event == "nudge" {
        return super::audio::wav(&nudge_pcm());
    }
    let notes: &[(f32, f32)] = match event {
        "online" => &[(523.25, 0.12), (659.25, 0.14), (783.99, 0.20)],
        "call" => &[(440.0, 0.25), (0.0, 0.12), (440.0, 0.25)],
        _ => &[(880.0, 0.10), (1174.66, 0.16)],
    };
    let mut samples = Vec::new();
    for (frequency, duration) in notes {
        let count = (duration * 16_000.0) as usize;
        for i in 0..count {
            let edge = (i.min(count - i) as f32 / 160.0).min(1.0);
            let decay = 1.0 - i as f32 / count as f32;
            let phase = TAU * frequency * i as f32 / 16_000.0;
            let wave = phase.sin() + (phase * 2.01).sin() * 0.28 + (phase * 3.99).sin() * 0.12;
            samples.push((wave * edge * decay * 5000.0) as i16);
        }
    }
    super::audio::wav(&samples)
}

#[cfg(test)]
mod tests {
    #[test]
    fn attention_cue_is_audible_bounded_and_distinct_from_a_regular_message() {
        let nudge = super::tone("nudge").unwrap();
        let message = super::tone("message").unwrap();
        let samples = crate::media::audio::decode_wav(&nudge).unwrap();
        assert!(samples.len() > 12000 && samples.len() < 20000);
        assert!(samples.iter().any(|sample| sample.unsigned_abs() > 8000));
        assert_ne!(nudge, message);
    }

    #[test]
    #[ignore = "requires physical audio device and not running concurrently in test harness"]
    #[cfg(feature = "native-audio")]
    fn default_device_resolves_and_plays_or_identifies_sound_server() {
        let tone = super::tone("message").unwrap();
        let res = crate::media::audio::play_notification(&tone, "");
        assert!(
            res.is_ok(),
            "play_notification with default device should succeed: {:?}",
            res
        );
    }
}
