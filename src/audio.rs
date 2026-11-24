//! Microphone capture, Opus encoding, mixing and playback.
//!
//! Everything runs internally at 48 kHz mono. Devices running at another
//! rate are converted with a simple, cheap linear resampler.

use crate::client::Net;
use crate::config::cfg;
use crate::protocol::lk;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const RATE: usize = 48000;
const FRAME: usize = 960; // 20 ms

struct Remote {
    buf: VecDeque<f32>,
    primed: bool,
    last_heard: Instant,
}

pub struct Mixer {
    remotes: HashMap<u32, Remote>,
    pub volumes: HashMap<u32, f32>,
    effects: VecDeque<f32>,
    pub deaf: bool,
}

impl Mixer {
    pub fn new() -> Self {
        Mixer { remotes: HashMap::new(), volumes: HashMap::new(), effects: VecDeque::new(), deaf: false }
    }

    pub fn push(&mut self, session: u32, pcm: &[f32]) {
        if self.deaf {
            return;
        }
        let r = self.remotes.entry(session).or_insert_with(|| Remote {
            buf: VecDeque::with_capacity(RATE / 4),
            primed: false,
            last_heard: Instant::now(),
        });
        r.buf.extend(pcm.iter().copied());
        r.last_heard = Instant::now();
        // Never let delay build up beyond ~250 ms.
        if r.buf.len() > RATE / 4 {
            let excess = r.buf.len() - RATE * 6 / 100;
            r.buf.drain(..excess);
        }
    }

    pub fn speaking(&self) -> Vec<u32> {
        self.remotes
            .iter()
            .filter(|(_, r)| r.last_heard.elapsed() < Duration::from_millis(300))
            .map(|(s, _)| *s)
            .collect()
    }

    pub fn retain_sessions(&mut self, keep: &HashSet<u32>) {
        self.remotes.retain(|s, _| keep.contains(s));
    }

    pub fn play_effect(&mut self, samples: &[f32]) {
        if self.effects.len() < RATE {
            self.effects.extend(samples.iter().copied());
        }
    }

    pub fn clear(&mut self) {
        self.remotes.clear();
        self.effects.clear();
        self.deaf = false;
    }

    fn next_sample(&mut self) -> f32 {
        let mut acc = 0.0f32;
        if !self.deaf {
            for (session, r) in self.remotes.iter_mut() {
                if !r.primed {
                    // Wait for 40 ms of audio to smooth out network jitter.
                    if r.buf.len() >= FRAME * 2 {
                        r.primed = true;
                    } else {
                        continue;
                    }
                }
                match r.buf.pop_front() {
                    Some(v) => acc += v * self.volumes.get(session).copied().unwrap_or(1.0),
                    None => r.primed = false,
                }
            }
        }
        if let Some(e) = self.effects.pop_front() {
            acc += e;
        }
        acc.clamp(-1.0, 1.0)
    }
}

// ---------- Sounds ----------

fn tones(parts: &[(f32, f32)]) -> Vec<f32> {
    let mut out = Vec::new();
    for &(freq, secs) in parts {
        let n = (secs * RATE as f32) as usize;
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let fade = (i.min(n - i) as f32 / 240.0).min(1.0);
            out.push((t * freq * std::f32::consts::TAU).sin() * 0.18 * fade);
        }
    }
    out
}

pub fn beep_on() -> Vec<f32> {
    tones(&[(880.0, 0.06)])
}
pub fn beep_off() -> Vec<f32> {
    tones(&[(587.0, 0.06)])
}
pub fn join_sound() -> Vec<f32> {
    tones(&[(659.0, 0.07), (988.0, 0.09)])
}
pub fn leave_sound() -> Vec<f32> {
    tones(&[(988.0, 0.07), (659.0, 0.09)])
}

// ---------- Devices ----------

pub fn list_devices(input: bool) -> Vec<String> {
    let host = cpal::default_host();
    let mut names = Vec::new();
    if input {
        if let Ok(devs) = host.input_devices() {
            names.extend(devs.filter_map(|d| d.name().ok()));
        }
    } else if let Ok(devs) = host.output_devices() {
        names.extend(devs.filter_map(|d| d.name().ok()));
    }
    names
}

fn find_device(host: &cpal::Host, name: &str, input: bool) -> Option<cpal::Device> {
    if !name.is_empty() {
        if input {
            if let Ok(devs) = host.input_devices() {
                for d in devs {
                    if d.name().ok().as_deref() == Some(name) {
                        return Some(d);
                    }
                }
            }
        } else if let Ok(devs) = host.output_devices() {
            for d in devs {
                if d.name().ok().as_deref() == Some(name) {
                    return Some(d);
                }
            }
        }
    }
    if input {
        host.default_input_device()
    } else {
        host.default_output_device()
    }
}

// ---------- Output ----------

fn build_output(device_name: &str, mixer: Arc<Mutex<Mixer>>) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = find_device(&host, device_name, false).ok_or("no speakers or headphones were found")?;
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let stream = match format {
        SampleFormat::F32 => out_stream::<f32>(&device, &config, mixer),
        SampleFormat::I16 => out_stream::<i16>(&device, &config, mixer),
        SampleFormat::U16 => out_stream::<u16>(&device, &config, mixer),
        SampleFormat::I32 => out_stream::<i32>(&device, &config, mixer),
        _ => Err("this audio device uses an unsupported format".to_string()),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

fn out_stream<T>(device: &cpal::Device, config: &cpal::StreamConfig, mixer: Arc<Mutex<Mixer>>) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels.max(1) as usize;
    let step = RATE as f64 / config.sample_rate.0.max(1) as f64;
    let mut frac = 0.0f64;
    let mut prev = 0.0f32;
    let mut cur = 0.0f32;
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                let mut m = lk(&mixer);
                for frame in data.chunks_mut(channels) {
                    while frac >= 1.0 {
                        prev = cur;
                        cur = m.next_sample();
                        frac -= 1.0;
                    }
                    let v = prev + (cur - prev) * frac as f32;
                    frac += step;
                    let s = T::from_sample(v);
                    for x in frame.iter_mut() {
                        *x = s;
                    }
                }
            },
            |e| eprintln!("speaker error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}

// ---------- Input ----------

fn build_input(device_name: &str, tx: mpsc::SyncSender<Vec<f32>>) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = find_device(&host, device_name, true).ok_or("no microphone was found")?;
    let supported = device.default_input_config().map_err(|e| e.to_string())?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let stream = match format {
        SampleFormat::F32 => in_stream::<f32>(&device, &config, tx),
        SampleFormat::I16 => in_stream::<i16>(&device, &config, tx),
        SampleFormat::U16 => in_stream::<u16>(&device, &config, tx),
        SampleFormat::I32 => in_stream::<i32>(&device, &config, tx),
        _ => Err("this microphone uses an unsupported format".to_string()),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

fn in_stream<T>(device: &cpal::Device, config: &cpal::StreamConfig, tx: mpsc::SyncSender<Vec<f32>>) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels.max(1) as usize;
    let step = config.sample_rate.0.max(1) as f64 / RATE as f64;
    let mut frac = 0.0f64;
    let mut prev = 0.0f32;
    let mut chunk: Vec<f32> = Vec::with_capacity(FRAME);
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                for frame in data.chunks(channels) {
                    let mut sum = 0.0f32;
                    for &s in frame {
                        sum += f32::from_sample(s);
                    }
                    let cur = sum / frame.len() as f32;
                    while frac < 1.0 {
                        chunk.push(prev + (cur - prev) * frac as f32);
                        frac += step;
                        if chunk.len() == FRAME {
                            let full = std::mem::replace(&mut chunk, Vec::with_capacity(FRAME));
                            let _ = tx.try_send(full);
                        }
                    }
                    frac -= 1.0;
                    prev = cur;
                }
            },
            |e| eprintln!("microphone error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}

fn spawn_encoder(rx: mpsc::Receiver<Vec<f32>>, net: Arc<Net>, mixer: Arc<Mutex<Mixer>>) {
    thread::spawn(move || {
        let mut enc = match opus::Encoder::new(48000, opus::Channels::Mono, opus::Application::Voip) {
            Ok(e) => e,
            Err(_) => return,
        };
        let _ = enc.set_bitrate(opus::Bitrate::Bits(32000));
        let mut out = [0u8; 1500];
        let mut seq: u16 = 0;
        let mut transmitting = false;
        let mut hang = 0u32;
        while let Ok(chunk) = rx.recv() {
            if !net.running() {
                break;
            }
            let energy: f32 = chunk.iter().map(|x| x * x).sum::<f32>() / chunk.len().max(1) as f32;
            let db = if energy > 1e-12 { 10.0 * energy.log10() } else { -100.0 };
            net.mic_level.store((db * 10.0) as i32, Ordering::Relaxed);

            let (va, key, threshold, beeps) = {
                let c = cfg();
                (c.voice_activation, c.ptt_key, c.va_threshold_db, c.ptt_beeps)
            };
            let want = if net.self_mute.load(Ordering::Relaxed) || net.deaf.load(Ordering::Relaxed) {
                hang = 0;
                false
            } else if va {
                if db > threshold as f32 {
                    hang = 20; // keep talking 400 ms after you go quiet
                    true
                } else if hang > 0 {
                    hang -= 1;
                    true
                } else {
                    false
                }
            } else {
                crate::ui::key_down(key)
            };

            if want != transmitting {
                transmitting = want;
                net.self_speaking.store(want, Ordering::Relaxed);
                if beeps && !va {
                    let s = if want { beep_on() } else { beep_off() };
                    lk(&mixer).play_effect(&s);
                }
                net.notify();
            }
            if transmitting {
                if let Ok(n) = enc.encode_float(&chunk, &mut out) {
                    net.send_voice(seq, &out[..n]);
                    seq = seq.wrapping_add(1);
                }
            }
        }
        net.self_speaking.store(false, Ordering::Relaxed);
    });
}

pub struct AudioEngine {
    _input: Option<cpal::Stream>,
    _output: Option<cpal::Stream>,
}

/// Opens the microphone and speakers. Returns warnings to show the user.
pub fn start(net: &Arc<Net>, mixer: &Arc<Mutex<Mixer>>) -> (AudioEngine, Vec<String>) {
    let (input_name, output_name) = {
        let c = cfg();
        (c.input_device.clone(), c.output_device.clone())
    };
    let mut warnings = Vec::new();
    let output = match build_output(&output_name, mixer.clone()) {
        Ok(s) => Some(s),
        Err(e) => {
            warnings.push(format!("Could not open your speakers/headphones: {e}"));
            None
        }
    };
    let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(16);
    let input = match build_input(&input_name, tx) {
        Ok(s) => {
            spawn_encoder(rx, net.clone(), mixer.clone());
            Some(s)
        }
        Err(e) => {
            warnings.push(format!("Could not open your microphone: {e}"));
            None
        }
    };
    (AudioEngine { _input: input, _output: output }, warnings)
}
