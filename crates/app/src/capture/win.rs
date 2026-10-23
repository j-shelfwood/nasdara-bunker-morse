use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use wasapi::{
    AudioCaptureClient, AudioClient, DeviceEnumerator, Direction, Handle, SampleType, StreamMode,
    WaveFormat, initialize_mta,
};
use windows::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};

use super::pacer::Pacer;
use super::{Event, SAMPLE_RATE, Source, Status};

const GAME_EXE: &str = "DayZ_x64.exe";
const PROBE_EVERY: Duration = Duration::from_secs(2);
const ALIVE_EVERY: Duration = Duration::from_millis(500);

pub fn run(source: Source, tx: Sender<Event>, stop: Arc<AtomicBool>) {
    if initialize_mta().is_err() {
        let _ = tx.send(Event::Status(Status::Failed(
            "Windows audio could not be initialised".into(),
        )));
        return;
    }
    let mut last: Option<Status> = None;
    let mut say = |s: Status| {
        if last.as_ref() != Some(&s) {
            last = Some(s.clone());
            let _ = tx.send(Event::Status(s));
        }
    };
    while !stop.load(Ordering::Relaxed) {
        let target = match source {
            Source::Game => match find_process(GAME_EXE) {
                Some(pid) => Some(pid),
                None => {
                    say(Status::WaitingForGame);
                    sleep_unless_stopped(&stop, PROBE_EVERY);
                    continue;
                }
            },
            Source::System => None,
        };
        match Session::open(target) {
            Ok(mut session) => {
                say(Status::Listening);
                let ended = session.pump(&tx, &stop, target);
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                say(match ended {
                    Ended::ProcessGone => Status::WaitingForGame,
                    Ended::DeviceError(e) => Status::Failed(e),
                });
            }
            Err(e) => say(Status::Failed(e)),
        }
        sleep_unless_stopped(&stop, PROBE_EVERY);
    }
}

enum Ended {
    ProcessGone,
    DeviceError(String),
}

struct Session {
    client: AudioClient,
    capture: AudioCaptureClient,
    event: Handle,
    scratch: Vec<u8>,
}

impl Session {
    /// A process id captures that process tree; none captures everything the default output plays.
    fn open(pid: Option<u32>) -> Result<Session, String> {
        let format = WaveFormat::new(16, 16, &SampleType::Int, SAMPLE_RATE, 2, None);
        let mode = StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: 0,
        };
        let mut client = match pid {
            Some(pid) => AudioClient::new_application_loopback_client(pid, true),
            None => DeviceEnumerator::new()
                .and_then(|e| e.get_default_device(&Direction::Render))
                .and_then(|d| d.get_iaudioclient()),
        }
        .map_err(|e| format!("No audio device: {e}"))?;
        client
            .initialize_client(&format, &Direction::Capture, &mode)
            .map_err(|e| format!("Audio capture refused: {e}"))?;
        let event = client.set_get_eventhandle().map_err(|e| e.to_string())?;
        let capture = client.get_audiocaptureclient().map_err(|e| e.to_string())?;
        client.start_stream().map_err(|e| e.to_string())?;
        Ok(Session {
            client,
            capture,
            event,
            scratch: vec![0; 8192 * 4],
        })
    }

    fn pump(&mut self, tx: &Sender<Event>, stop: &AtomicBool, pid: Option<u32>) -> Ended {
        let mut pacer = Pacer::new(Instant::now());
        let mut next_alive = Instant::now() + ALIVE_EVERY;
        let ended = loop {
            if stop.load(Ordering::Relaxed) {
                break Ended::ProcessGone;
            }
            if let Err(e) = self.drain(&mut pacer) {
                break Ended::DeviceError(format!("Audio capture stopped: {e}"));
            }
            let now = Instant::now();
            if let Some(pid) = pid
                && now >= next_alive
            {
                next_alive = now + ALIVE_EVERY;
                if !is_alive(pid) {
                    break Ended::ProcessGone;
                }
            }
            let due = pacer.due(now);
            if !due.is_empty() && tx.send(Event::Samples(due)).is_err() {
                break Ended::ProcessGone;
            }
            // wakes on new audio; times out when nothing plays, which is when the pacer supplies the silence
            let _ = self.event.wait_for_event(10);
        };
        let _ = self.client.stop_stream();
        ended
    }

    fn drain(&mut self, pacer: &mut Pacer) -> Result<(), String> {
        loop {
            let frames = self
                .capture
                .get_next_packet_size()
                .map_err(|e| e.to_string())?
                .unwrap_or(0) as usize;
            if frames == 0 {
                return Ok(());
            }
            if self.scratch.len() < frames * 4 {
                self.scratch.resize(frames * 4, 0);
            }
            let (got, info) = self
                .capture
                .read_from_device(&mut self.scratch)
                .map_err(|e| e.to_string())?;
            let got = got as usize;
            if info.flags.silent {
                pacer.push(std::iter::repeat_n(0, got));
                continue;
            }
            pacer.push(self.scratch[..got * 4].as_chunks::<4>().0.iter().map(|f| {
                let l = i16::from_le_bytes([f[0], f[1]]) as i32;
                let r = i16::from_le_bytes([f[2], f[3]]) as i32;
                ((l + r) / 2) as i16
            }));
        }
    }
}

fn sleep_unless_stopped(stop: &AtomicBool, total: Duration) {
    let until = Instant::now() + total;
    while Instant::now() < until && !stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn find_process(exe: &str) -> Option<u32> {
    // SAFETY: the snapshot handle is closed on every path; the entry struct is initialised with its size as the API requires.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut found = None;
        let mut ok = Process32FirstW(snapshot, &mut entry).is_ok();
        while ok {
            let len = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            if String::from_utf16_lossy(&entry.szExeFile[..len]).eq_ignore_ascii_case(exe) {
                found = Some(entry.th32ProcessID);
                break;
            }
            ok = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
        found
    }
}

fn is_alive(pid: u32) -> bool {
    // SAFETY: the process handle is closed before returning.
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut code = 0u32;
        let alive = GetExitCodeProcess(handle, &mut code).is_ok() && code == STILL_ACTIVE.0 as u32;
        let _ = CloseHandle(handle);
        alive
    }
}
