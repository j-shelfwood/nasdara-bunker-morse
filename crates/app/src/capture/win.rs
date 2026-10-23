use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use wasapi::{
    AudioCaptureClient, AudioClient, DeviceEnumerator, Direction, Handle, SampleType, StreamMode,
    WaveFormat, initialize_mta,
};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject};
use windows::core::HRESULT;

use super::InFlight;
use super::pacer::Pacer;
use super::{Event, SAMPLE_RATE, Sink, Source, Status, failure_message, retry_after};

const GAME_EXE: &str = "DayZ_x64.exe";
const PROBE_EVERY: Duration = Duration::from_secs(2);
const ALIVE_EVERY: Duration = Duration::from_millis(500);

pub fn run(source: Source, tx: Sink, stop: Arc<AtomicBool>) {
    if initialize_mta().is_err() {
        tx.send(Event::Status(Status::Failed(
            "Windows audio could not be initialised".into(),
        )));
        return;
    }
    let mut last: Option<Status> = None;
    let mut say = |s: Status| {
        if last.as_ref() != Some(&s) {
            last = Some(s.clone());
            tx.send(Event::Status(s));
        }
    };
    while !stop.load(Ordering::Relaxed) {
        let target = match source {
            Source::Game => match find_process(GAME_EXE) {
                Some(process) => Some(process),
                None => {
                    say(Status::WaitingForGame);
                    sleep_unless_stopped(&stop, PROBE_EVERY);
                    continue;
                }
            },
            Source::System => None,
        };
        match Session::open(target.as_ref().map(|p| p.pid), &stop) {
            Ok(mut session) => {
                say(Status::Listening);
                let ended = session.pump(&tx, &stop, target.as_ref());
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                tx.send(Event::SessionEnded);
                match ended {
                    Ended::ProcessGone => say(Status::WaitingForGame),
                    Ended::DeviceError(e) => say(Status::Failed(e)),
                    // the default output moved: listen again on the new one right away
                    Ended::DeviceChanged => continue,
                }
            }
            Err(e) => {
                say(Status::Failed(failure_message(source, &e)));
                sleep_unless_stopped(&stop, retry_after(source));
                continue;
            }
        }
        sleep_unless_stopped(&stop, PROBE_EVERY);
    }
}

enum Ended {
    ProcessGone,
    DeviceChanged,
    DeviceError(String),
}

struct Session {
    client: AudioClient,
    capture: AudioCaptureClient,
    event: Handle,
    scratch: Vec<u8>,
    /// Id of the default render device being captured (system mode); a change of default is not an error for
    /// Windows, the old device just keeps working, so it is polled.
    device: Option<String>,
}

impl Session {
    /// A process id captures that process tree; none captures everything the default output plays.
    fn open(pid: Option<u32>, stop: &AtomicBool) -> Result<Session, String> {
        let format = WaveFormat::new(16, 16, &SampleType::Int, SAMPLE_RATE, 2, None);
        let mode = StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: 0,
        };
        let mut device = None;
        let mut client = match pid {
            Some(pid) => activate_loopback(pid, stop),
            None => DeviceEnumerator::new()
                .and_then(|e| e.get_default_device(&Direction::Render))
                .and_then(|d| {
                    device = d.get_id().ok();
                    d.get_iaudioclient()
                })
                .map_err(|e| e.to_string()),
        }?;
        client
            .initialize_client(&format, &Direction::Capture, &mode)
            .map_err(|e| format!("initialize: {e}"))?;
        let event = client.set_get_eventhandle().map_err(|e| e.to_string())?;
        let capture = client.get_audiocaptureclient().map_err(|e| e.to_string())?;
        client.start_stream().map_err(|e| e.to_string())?;
        Ok(Session {
            client,
            capture,
            event,
            scratch: vec![0; 8192 * 4],
            device,
        })
    }

    fn pump(&mut self, tx: &Sink, stop: &AtomicBool, process: Option<&Process>) -> Ended {
        let mut pacer = Pacer::new(Instant::now());
        let mut next_alive = Instant::now() + ALIVE_EVERY;
        let mut next_device = Instant::now() + PROBE_EVERY;
        let ended = loop {
            if stop.load(Ordering::Relaxed) {
                break Ended::ProcessGone;
            }
            match self.drain(&mut pacer) {
                Ok(0) => {}
                Ok(gaps) => {
                    if !tx.send(Event::Discontinuity(gaps)) {
                        break Ended::ProcessGone;
                    }
                }
                Err(e) => break Ended::DeviceError(format!("Audio capture stopped: {e}")),
            }
            let now = Instant::now();
            if let Some(process) = process
                && now >= next_alive
            {
                next_alive = now + ALIVE_EVERY;
                if !process.alive() {
                    break Ended::ProcessGone;
                }
            }
            if let Some(device) = &self.device
                && now >= next_device
            {
                next_device = now + PROBE_EVERY;
                if default_render_id().is_some_and(|id| &id != device) {
                    break Ended::DeviceChanged;
                }
            }
            let due = pacer.due(now);
            if !due.is_empty() && !tx.send_samples(due) {
                break Ended::ProcessGone;
            }
            // wakes on new audio, or when the next chunk is due, which is how the pacer supplies silence
            if !self.wait(pacer.until_next(Instant::now())) {
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        let _ = self.client.stop_stream();
        ended
    }

    /// Waits for new audio for at most `limit` (and at most 10 ms). `false` when the wait itself failed, which is
    /// different from timing out: the caller must not spin on it.
    fn wait(&self, limit: Duration) -> bool {
        let ms = limit.as_micros().div_ceil(1000).clamp(1, 10) as u32;
        // SAFETY: the event handle is owned by `self.event`, which outlives the call.
        let r = unsafe { WaitForSingleObject(self.event.as_handle(), ms) };
        r == WAIT_OBJECT_0 || r == WAIT_TIMEOUT
    }

    /// Moves every waiting packet into the pacer; returns how many were flagged as discontinuous.
    fn drain(&mut self, pacer: &mut Pacer) -> Result<u32, String> {
        let mut gaps = 0;
        loop {
            let frames = self
                .capture
                .get_next_packet_size()
                .map_err(|e| e.to_string())?
                .unwrap_or(0) as usize;
            if frames == 0 {
                return Ok(gaps);
            }
            if self.scratch.len() < frames * 4 {
                self.scratch.resize(frames * 4, 0);
            }
            let (got, info) = self
                .capture
                .read_from_device(&mut self.scratch)
                .map_err(|e| e.to_string())?;
            let got = got as usize;
            gaps += u32::from(info.flags.data_discontinuity);
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

fn default_render_id() -> Option<String> {
    DeviceEnumerator::new()
        .and_then(|e| e.get_default_device(&Direction::Render))
        .and_then(|d| d.get_id())
        .ok()
}

/// How long Windows may take to answer the per-process activation before the attempt is abandoned.
const ACTIVATION_TIMEOUT: Duration = Duration::from_secs(5);

/// The activated client crossing back from the helper thread.
struct Activated(AudioClient);

// SAFETY: both threads live in the multithreaded COM apartment, which has no thread affinity, and the client comes
// out of ActivateAudioInterfaceAsync, whose result objects are free-threaded. It is used by one thread at a time.
unsafe impl Send for Activated {}

/// `AudioClient::new_application_loopback_client` waits on a condition variable without a timeout, so it runs on a
/// helper thread. A call that never returns leaks that thread and nothing else; its late result is dropped.
fn activate_loopback(pid: u32, stop: &AtomicBool) -> Result<AudioClient, String> {
    static ACTIVATING: InFlight = InFlight::new();
    let Some(in_flight) = ACTIVATING.begin() else {
        return Err("an earlier request to Windows has not returned".into());
    };
    let (done, result) = std::sync::mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("activate".into())
        .spawn(move || {
            let _in_flight = in_flight;
            let client = if initialize_mta().is_err() {
                Err("Windows audio could not be initialised".to_string())
            } else {
                AudioClient::new_application_loopback_client(pid, true)
                    .map(Activated)
                    .map_err(|e| e.to_string())
            };
            let _ = done.send(client);
        });
    if let Err(e) = spawned {
        return Err(e.to_string());
    }
    let until = Instant::now() + ACTIVATION_TIMEOUT;
    while Instant::now() < until && !stop.load(Ordering::Relaxed) {
        match result.recv_timeout(Duration::from_millis(100)) {
            Ok(client) => return client.map(|a| a.0),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    Err("Windows did not answer within 5 seconds".into())
}

fn sleep_unless_stopped(stop: &AtomicBool, total: Duration) {
    let until = Instant::now() + total;
    while Instant::now() < until && !stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The game process, found by name. The handle stays open for the session: waiting on it tells when the process
/// ended, and a held handle keeps Windows from giving its id to another process.
struct Process {
    pid: u32,
    /// `None` when Windows refuses to open the process (it runs elevated): liveness then comes from the process list.
    handle: Option<HANDLE>,
}

impl Process {
    fn alive(&self) -> bool {
        let Some(handle) = self.handle else {
            // not openable (elevated): a process-loopback client outlives its target, so ask the process list
            return find_pid(GAME_EXE) == Some(self.pid);
        };
        // SAFETY: the handle is owned by this value and open until it is dropped.
        unsafe { WaitForSingleObject(handle, 0) == WAIT_TIMEOUT }
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        if let Some(handle) = self.handle {
            // SAFETY: the handle came from OpenProcess and is closed only here.
            let _ = unsafe { CloseHandle(handle) };
        }
    }
}

fn find_process(exe: &str) -> Option<Process> {
    let pid = find_pid(exe)?;
    // SAFETY: plain call; the returned handle is owned by the `Process`.
    match unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) } {
        Ok(handle) => Some(Process {
            pid,
            handle: Some(handle),
        }),
        Err(e) if e.code() == HRESULT::from_win32(ERROR_ACCESS_DENIED.0) => {
            Some(Process { pid, handle: None })
        }
        // gone between the snapshot and now
        Err(_) => None,
    }
}

fn find_pid(exe: &str) -> Option<u32> {
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
