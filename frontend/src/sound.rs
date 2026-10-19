//! the title's sound through ALSA, stretched in time to keep the device's
//! buffer about a tenth of a second full whatever the frame rate.

use std::sync::mpsc;

use zakuro_core::AUDIO_SAMPLE_RATE;

pub fn play(samples: mpsc::Receiver<Vec<[i16; 2]>>) {
    std::thread::spawn(move || {
        // SAFETY: plain ALSA calls on the device this opens, with buffers
        // that live as long as each call
        unsafe {
            let Ok(alsa) = libloading::Library::new("libasound.so.2") else {
                log::warn!("no libasound, playing without sound");
                for _ in samples {}
                return;
            };
            let open: libloading::Symbol<unsafe extern "C" fn(*mut *mut u8, *const u8, i32, i32) -> i32> = alsa.get(b"snd_pcm_open\0").unwrap();
            let set: libloading::Symbol<unsafe extern "C" fn(*mut u8, i32, i32, u32, u32, i32, u32) -> i32> = alsa.get(b"snd_pcm_set_params\0").unwrap();
            let write: libloading::Symbol<unsafe extern "C" fn(*mut u8, *const u8, u64) -> i64> = alsa.get(b"snd_pcm_writei\0").unwrap();
            let recover: libloading::Symbol<unsafe extern "C" fn(*mut u8, i32, i32) -> i32> = alsa.get(b"snd_pcm_recover\0").unwrap();
            let delay: libloading::Symbol<unsafe extern "C" fn(*mut u8, *mut i64) -> i32> = alsa.get(b"snd_pcm_delay\0").unwrap();
            let get: libloading::Symbol<unsafe extern "C" fn(*mut u8, *mut u64, *mut u64) -> i32> = alsa.get(b"snd_pcm_get_params\0").unwrap();
            let mut pcm = std::ptr::null_mut();
            if open(&mut pcm, b"default\0".as_ptr(), 0, 0) < 0 || set(pcm, 2, 3, 2, AUDIO_SAMPLE_RATE.round() as u32, 1, 150_000) < 0 {
                log::warn!("could not open the sound device, playing without sound");
                for _ in samples {}
                return;
            }
            let (mut buffer, mut period) = (0, 0);
            get(pcm, &mut buffer, &mut period);
            log::info!("sound at {} Hz, the device buffering {buffer} samples in periods of {period}", AUDIO_SAMPLE_RATE.round());
            let target = AUDIO_SAMPLE_RATE * 0.15;
            // over a second behind, drop the oldest
            let most = AUDIO_SAMPLE_RATE as usize;
            let mut stretch = Stretch::default();
            let mut ratio = 1.0f64;
            let mut chunk = Vec::new();
            let mut writes = 0u32;
            while let Ok(mut input) = samples.recv() {
                while let Ok(more) = samples.try_recv() {
                    input.extend(more);
                }
                let mut queued = 0i64;
                if delay(pcm, &mut queued) < 0 {
                    queued = 0;
                }
                let waiting = stretch.waiting() + input.len() + queued.max(0) as usize;
                if waiting > most {
                    let over = (waiting - most).min(input.len());
                    input.drain(..over);
                    log::info!("sound fell {:.1} s behind, dropped {:.1} s of it", waiting as f64 / AUDIO_SAMPLE_RATE, over as f64 / AUDIO_SAMPLE_RATE);
                }
                let waiting = stretch.waiting() + input.len() + queued.max(0) as usize;
                writes += 1;
                if writes == 300 {
                    log::info!("sound {queued} samples in the device, {waiting} waiting all told, played {ratio:.2} times as long");
                }
                let wanted = (1.0 + (target - waiting as f64) / target).clamp(0.75, 2.5);
                ratio += (wanted - ratio) * 0.1;
                chunk.clear();
                stretch.push(&input, ratio, &mut chunk);
                let mut done = 0;
                while done < chunk.len() {
                    let written = write(pcm, chunk[done..].as_ptr().cast(), (chunk.len() - done) as u64);
                    if written < 0 {
                        if recover(pcm, written as i32, 1) < 0 {
                            break;
                        }
                    } else {
                        done += written as usize;
                    }
                }
            }
        }
    });
}

const SEGMENT: usize = 1024;
const OVERLAP: usize = 256;
const SEEK: usize = 192;
const HOP: usize = SEGMENT - OVERLAP;

/// stretches sound in time without changing its pitch, WSOLA: segments of
/// the input are laid one after another, each moved within a small window to
/// where it best continues the last and crossfaded into it.
#[derive(Default)]
struct Stretch {
    input: Vec<[f32; 2]>,
    at: f64,
    tail: Vec<[f32; 2]>,
}

impl Stretch {
    fn waiting(&self) -> usize {
        self.input.len().saturating_sub(self.at as usize)
    }

    fn push(&mut self, samples: &[[i16; 2]], ratio: f64, out: &mut Vec<[i16; 2]>) {
        self.input.extend(samples.iter().map(|&[l, r]| [l as f32, r as f32]));
        let step = HOP as f64 / ratio;
        loop {
            let base = self.at.round() as usize;
            if base + SEEK + SEGMENT > self.input.len() {
                break;
            }
            let mut best = (f32::MIN, base);
            if self.tail.is_empty() {
                best.1 = base;
            } else {
                for start in (base.saturating_sub(SEEK)..=base + SEEK).step_by(4) {
                    let score: f32 = (0..OVERLAP)
                        .step_by(2)
                        .map(|i| {
                            let (a, b) = (self.tail[i], self.input[start + i]);
                            (a[0] + a[1]) * (b[0] + b[1])
                        })
                        .sum();
                    if score > best.0 {
                        best = (score, start);
                    }
                }
            }
            let start = best.1;
            for i in 0..HOP {
                let sample = self.input[start + i];
                let mixed = if i < OVERLAP && !self.tail.is_empty() {
                    let w = i as f32 / OVERLAP as f32;
                    [self.tail[i][0] * (1.0 - w) + sample[0] * w, self.tail[i][1] * (1.0 - w) + sample[1] * w]
                } else {
                    sample
                };
                out.push(mixed.map(|value| value.clamp(-32768.0, 32767.0) as i16));
            }
            self.tail = self.input[start + HOP..start + SEGMENT].to_vec();
            self.at += step;
        }
        let used = (self.at as usize).saturating_sub(SEEK);
        if used > 0 {
            self.input.drain(..used);
            self.at -= used as f64;
        }
    }
}
