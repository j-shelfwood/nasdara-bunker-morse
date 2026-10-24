//! Plays the test-mode clip through the default output, so the simulated broadcast can be heard as well as seen.
//! Only Windows has an output; elsewhere the speaker accepts audio and drops it.

#[cfg(any(windows, test))]
use std::collections::VecDeque;

/// Audio kept back before playback starts, and again after a gap, so a late chunk does not crackle.
#[cfg(any(windows, test))]
const PRIME_S: f64 = 0.15;
/// The most audio held for a stalled device; older audio is dropped.
#[cfg(any(windows, test))]
const MAX_S: f64 = 2.0;

/// Mono samples waiting for the device, handed out as stereo 16-bit frames.
#[cfg(any(windows, test))]
struct Backlog {
    samples: VecDeque<i16>,
    playing: bool,
    rate: usize,
}

#[cfg(any(windows, test))]
impl Backlog {
    fn new(rate: usize) -> Backlog {
        Backlog {
            samples: VecDeque::new(),
            playing: false,
            rate,
        }
    }

    fn push(&mut self, pcm: &[i16]) {
        self.samples.extend(pcm);
        let cap = (MAX_S * self.rate as f64) as usize;
        if self.samples.len() > cap {
            self.samples.drain(..self.samples.len() - cap);
        }
    }

    /// `frames` stereo frames as little-endian bytes: the waiting audio once enough has gathered, silence before
    /// that and after it runs dry.
    fn fill(&mut self, frames: usize) -> Vec<u8> {
        if !self.playing && self.samples.len() >= (PRIME_S * self.rate as f64) as usize {
            self.playing = true;
        }
        let mut out = Vec::with_capacity(frames * 4);
        for _ in 0..frames {
            let s = if self.playing {
                self.samples.pop_front()
            } else {
                None
            };
            if s.is_none() {
                self.playing = false;
            }
            let b = s.unwrap_or(0).to_le_bytes();
            out.extend_from_slice(&[b[0], b[1], b[0], b[1]]);
        }
        out
    }
}

#[cfg(all(windows, not(test)))]
pub use win::Speaker;

#[cfg(not(all(windows, not(test))))]
pub struct Speaker;

#[cfg(not(all(windows, not(test))))]
impl Speaker {
    pub fn open() -> Speaker {
        Speaker
    }

    pub fn play(&mut self, _pcm: &[i16]) {}
}

#[cfg(all(windows, not(test)))]
mod win {
    use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

    use wasapi::{
        AudioClient, AudioRenderClient, DeviceEnumerator, Direction, Handle, SampleType,
        StreamMode, WaveFormat, initialize_mta,
    };

    use super::Backlog;
    use crate::capture::SAMPLE_RATE;

    /// The audio is handed to a thread that owns the output, so the paced demo loop never waits on Windows.
    pub struct Speaker {
        tx: Option<Sender<Vec<i16>>>,
    }

    impl Speaker {
        /// A speaker on the default output. When none can be opened it stays silent and the demo carries on.
        pub fn open() -> Speaker {
            let (tx, rx) = channel();
            let spawned = std::thread::Builder::new()
                .name("speaker".into())
                .spawn(move || render(rx));
            Speaker {
                tx: spawned.ok().map(|_| tx),
            }
        }

        pub fn play(&mut self, pcm: &[i16]) {
            if self
                .tx
                .as_ref()
                .is_some_and(|t| t.send(pcm.to_vec()).is_err())
            {
                self.tx = None;
            }
        }
    }

    struct Output {
        client: AudioClient,
        render: AudioRenderClient,
        event: Handle,
    }

    impl Output {
        fn open() -> Result<Output, String> {
            let format = WaveFormat::new(16, 16, &SampleType::Int, SAMPLE_RATE, 2, None);
            let mode = StreamMode::EventsShared {
                autoconvert: true,
                buffer_duration_hns: 0,
            };
            let mut client = DeviceEnumerator::new()
                .and_then(|e| e.get_default_device(&Direction::Render))
                .and_then(|d| d.get_iaudioclient())
                .map_err(|e| e.to_string())?;
            client
                .initialize_client(&format, &Direction::Render, &mode)
                .map_err(|e| format!("initialize: {e}"))?;
            let event = client.set_get_eventhandle().map_err(|e| e.to_string())?;
            let render = client.get_audiorenderclient().map_err(|e| e.to_string())?;
            Ok(Output {
                client,
                render,
                event,
            })
        }

        /// Fills the free part of the device buffer from the backlog.
        fn feed(&self, backlog: &mut Backlog) -> Result<(), String> {
            let frames = self
                .client
                .get_available_space_in_frames()
                .map_err(|e| e.to_string())? as usize;
            self.render
                .write_to_device(frames, &backlog.fill(frames), None)
                .map_err(|e| e.to_string())
        }
    }

    fn render(rx: Receiver<Vec<i16>>) {
        if initialize_mta().is_err() {
            return;
        }
        let Ok(output) = Output::open() else { return };
        let mut backlog = Backlog::new(SAMPLE_RATE);
        if output.feed(&mut backlog).is_err() || output.client.start_stream().is_err() {
            return;
        }
        loop {
            // wakes when the device wants more; the timeout only lets a closed channel end the thread
            let _ = output.event.wait_for_event(100);
            loop {
                match rx.try_recv() {
                    Ok(pcm) => backlog.push(&pcm),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        let _ = output.client.stop_stream();
                        return;
                    }
                }
            }
            if output.feed(&mut backlog).is_err() {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: usize = 1000;

    fn frames(bytes: &[u8]) -> Vec<(i16, i16)> {
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|f| {
                (
                    i16::from_le_bytes([f[0], f[1]]),
                    i16::from_le_bytes([f[2], f[3]]),
                )
            })
            .collect()
    }

    #[test]
    fn nothing_plays_until_a_little_audio_has_gathered() {
        let mut b = Backlog::new(RATE);
        b.push(&[7; 100]);
        assert!(frames(&b.fill(10)).iter().all(|&f| f == (0, 0)));
        b.push(&[7; 50]);
        assert!(frames(&b.fill(10)).iter().all(|&f| f == (7, 7)));
    }

    #[test]
    fn both_channels_carry_the_mono_sample_in_order() {
        let mut b = Backlog::new(RATE);
        let pcm: Vec<i16> = (0..200).map(|i| i * 100 - 9_000).collect();
        b.push(&pcm);
        let got = frames(&b.fill(200));
        assert_eq!(got.len(), 200);
        assert!(got.iter().zip(&pcm).all(|(f, &s)| *f == (s, s)));
    }

    #[test]
    fn running_dry_gives_silence_and_waits_for_new_audio_to_gather() {
        let mut b = Backlog::new(RATE);
        b.push(&[5; 200]);
        let got = frames(&b.fill(300));
        assert!(got[..200].iter().all(|&f| f == (5, 5)));
        assert!(got[200..].iter().all(|&f| f == (0, 0)));
        b.push(&[5; 20]);
        assert!(frames(&b.fill(20)).iter().all(|&f| f == (0, 0)));
    }

    #[test]
    fn a_stalled_device_keeps_only_the_latest_audio() {
        let mut b = Backlog::new(RATE);
        let old = vec![1i16; 3000];
        b.push(&old);
        b.push(&[2; 500]);
        assert_eq!(b.samples.len(), (MAX_S * RATE as f64) as usize);
        assert_eq!(b.samples.back(), Some(&2));
        assert_eq!(b.samples.front(), Some(&1));
    }
}
