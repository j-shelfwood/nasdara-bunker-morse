use std::path::PathBuf;

use crate::capture::{Capture, Event, Source};
use crate::engine::{self, Handle};
use crate::log::DebugLog;
use crate::ui::{self, Settings, UiState};

const APP_ID: &str = "nasdara-bunker-morse";

struct Options {
    log: Option<PathBuf>,
    trace: bool,
    /// `--probe <seconds> <report file> [system]`: listen without a window and write what was heard, for testing capture.
    probe: Option<(u64, PathBuf, Source)>,
    /// `--gpu-check <report file>`: list the graphics adapters the window could use, for diagnosing a window that will not open.
    gpu_check: Option<PathBuf>,
    /// `--demo`: play a synthetic transmission on repeat instead of listening. The default where live capture does not
    /// exist, so the window can be tried and reviewed off Windows.
    demo: bool,
}

fn options() -> Options {
    let mut o = Options {
        log: None,
        trace: false,
        probe: None,
        gpu_check: None,
        demo: cfg!(not(windows)),
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--log" => o.log = args.next().map(PathBuf::from),
            "--trace" => o.trace = true,
            "--demo" => o.demo = true,
            "--gpu-check" => o.gpu_check = args.next().map(PathBuf::from),
            "--probe" => {
                let seconds = args.next().and_then(|v| v.parse().ok()).unwrap_or(10);
                let report = args
                    .next()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| "probe.txt".into());
                let source = if args.next().as_deref() == Some("system") {
                    Source::System
                } else {
                    Source::Game
                };
                o.probe = Some((seconds, report, source));
            }
            _ => {}
        }
    }
    o
}

pub fn run() -> eframe::Result {
    install_panic_hook();
    let opts = options();
    if let Some(report) = &opts.gpu_check {
        gpu_check(report);
        return Ok(());
    }
    if let Some((seconds, report, source)) = &opts.probe {
        probe(*seconds, report, *source);
        return Ok(());
    }
    let icon = egui::IconData {
        rgba: crate::logo::rasterize(64),
        width: 64,
        height: 64,
    };
    lower_priority();
    // prefer the integrated GPU where there is one, and leave the fast one to the game; DX12 is the predictable choice on Windows
    let mut gpu = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    gpu.power_preference = eframe::wgpu::PowerPreference::LowPower;
    #[cfg(windows)]
    {
        gpu.instance_descriptor.backends = eframe::wgpu::Backends::DX12;
    }
    let native = eframe::NativeOptions {
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(gpu),
            ..Default::default()
        },
        viewport: egui::ViewportBuilder::default()
            .with_title("Bunker Morse")
            .with_inner_size([1060.0, 800.0])
            .with_min_inner_size([760.0, 520.0])
            .with_icon(icon),
        ..Default::default()
    };
    let result = eframe::run_native(
        APP_ID,
        native,
        Box::new(move |cc| {
            crate::theme::install(&cc.egui_ctx);
            let settings: Settings = cc
                .storage
                .and_then(|s| eframe::get_value(s, "settings"))
                .unwrap_or_default();
            let log = opts.log.map(|p| DebugLog::new(p, opts.trace));
            let engine = engine::start(settings.source, cc.egui_ctx.clone(), log);
            if opts.demo {
                engine.send(engine::Command::StartDemo);
            }
            Ok(Box::new(App {
                engine,
                st: UiState::new(settings),
            }))
        }),
    );
    if let Err(e) = &result {
        report_failure(&format!("Bunker Morse could not open its window.\n\n{e}"));
    }
    result
}

#[cfg(windows)]
fn lower_priority() {
    use windows::Win32::System::Threading::{
        BELOW_NORMAL_PRIORITY_CLASS, GetCurrentProcess, SetPriorityClass,
    };
    // SAFETY: the pseudo handle of the current process is always valid.
    unsafe {
        let _ = SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
    }
}

#[cfg(not(windows))]
fn lower_priority() {}

/// The exe has no console, so a start-up failure would otherwise vanish without a trace.
#[cfg(windows)]
fn report_failure(message: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    use windows::core::HSTRING;
    // SAFETY: both strings outlive the call; no owner window.
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(message),
            &HSTRING::from("Bunker Morse"),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(windows))]
fn report_failure(message: &str) {
    eprintln!("{message}");
}

/// Release builds abort on panic and the exe has no console, so without this the window would just vanish.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        previous(info);
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "no message".into());
        let location = info.location().map_or_else(
            || "unknown".into(),
            |l| format!("{}:{}", l.file(), l.line()),
        );
        let thread = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_owned();
        let text = crash_report(&message, &location, &thread);
        let dir = eframe::storage_dir(APP_ID).unwrap_or_else(std::env::temp_dir);
        let shown = match write_crash(&dir, &text) {
            Some(path) => format!("\n\nDetails were saved to {}", path.display()),
            None => String::new(),
        };
        report_failure(&format!(
            "Bunker Morse stopped because of an internal error.\n\n{message}{shown}"
        ));
    }));
}

/// Message and location only: no audio, no decoded data.
fn crash_report(message: &str, location: &str, thread: &str) -> String {
    format!(
        "Bunker Morse {} crashed\nthread: {thread}\nlocation: {location}\nmessage: {message}\n",
        env!("CARGO_PKG_VERSION")
    )
}

/// Writes `crash.txt` into `dir` (created when missing), falling back to nothing when the disk refuses.
fn write_crash(dir: &std::path::Path, text: &str) -> Option<PathBuf> {
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join("crash.txt");
    std::fs::write(&path, text).ok().map(|()| path)
}

struct App {
    engine: Handle,
    st: UiState,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui::draw(ui, &self.engine, &mut self.st);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "settings", &self.st.settings);
    }
}

fn probe(seconds: u64, report: &PathBuf, source: Source) {
    use std::io::Write;
    use std::time::{Duration, Instant};

    let (tx, rx) = std::sync::mpsc::sync_channel(crate::capture::CHANNEL_CAPACITY);
    let _capture = Capture::live(source, 0, tx);
    let started = Instant::now();
    let (mut samples, mut peak, mut statuses) = (0usize, 0i16, Vec::new());
    let mut stream_seconds = Vec::new();
    while started.elapsed() < Duration::from_secs(seconds) {
        match rx.recv_timeout(Duration::from_millis(100)).map(|m| m.event) {
            Ok(Event::Status(s)) => {
                statuses.push(format!("{:.1}s {s:?}", started.elapsed().as_secs_f32()))
            }
            Ok(Event::Samples(pcm)) => {
                samples += pcm.len();
                peak = peak.max(pcm.iter().map(|s| s.saturating_abs()).max().unwrap_or(0));
                if stream_seconds.len() < 64 && samples / 44_100 >= stream_seconds.len() {
                    stream_seconds.push(started.elapsed().as_secs_f32());
                }
            }
            _ => {}
        }
    }
    if let Ok(mut f) = std::fs::File::create(report) {
        let _ = writeln!(
            f,
            "source {source:?}\nwall {:.2}s\nsamples {samples} ({:.2}s of audio)\npeak {peak}",
            started.elapsed().as_secs_f32(),
            samples as f64 / 44_100.0
        );
        for s in statuses {
            let _ = writeln!(f, "status {s}");
        }
    }
}

fn gpu_check(report: &PathBuf) {
    let instance =
        eframe::wgpu::Instance::new(eframe::wgpu::InstanceDescriptor::new_without_display_handle());
    let adapters = pollster::block_on(instance.enumerate_adapters(eframe::wgpu::Backends::all()));
    let mut text = format!("{} adapter(s)\n", adapters.len());
    for a in &adapters {
        let i = a.get_info();
        text += &format!("{:?} | {} | {:?}\n", i.backend, i.name, i.device_type);
    }
    let _ = std::fs::write(report, text);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crash_file_holds_message_and_location_and_lands_in_the_directory() {
        let dir = std::env::temp_dir().join(format!("bunker-morse-crash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let text = crash_report("index out of bounds", "src/engine.rs:42", "engine");
        let path = write_crash(&dir.join("nested"), &text).expect("written");
        let read = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(path.ends_with("crash.txt"));
        assert!(read.contains("message: index out of bounds"), "{read}");
        assert!(read.contains("location: src/engine.rs:42"), "{read}");
        assert!(read.contains("thread: engine"), "{read}");
    }
}
