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
}

fn options() -> Options {
    let mut o = Options {
        log: None,
        trace: false,
        probe: None,
        gpu_check: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--log" => o.log = args.next().map(PathBuf::from),
            "--trace" => o.trace = true,
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
            .with_min_inner_size([900.0, 620.0])
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
