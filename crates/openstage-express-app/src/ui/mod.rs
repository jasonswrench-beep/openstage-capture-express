// SPDX-License-Identifier: Apache-2.0
//! The window (milestone M5): Home and the Editor, over an example guide, drawn with egui and described to screen readers
//! through AccessKit.
//!
//! The screens (`home`, `editor`) and their state (`editor_state`, `rows`) draw into a `Ui` they are given and know nothing of
//! the operating system, so the tests run whole screens without opening a window and read back what a screen reader would be
//! told. This file is the only place that opens a real window.

pub mod app;
pub mod editor;
pub mod editor_state;
pub mod fonts;
pub mod home;
pub mod rows;
pub mod theme;
pub mod widgets;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;

pub use app::{App, Screen};

/// Switches for looking at the window without a person at it (used by the checks, and documented in `--help`).
#[derive(Debug, Clone, Default)]
pub struct DevOptions {
    /// Start on this screen instead of Home.
    pub screen: Option<Screen>,
    /// Save a picture of the window to this PNG file, then close it.
    pub screenshot: Option<PathBuf>,
    /// Close the window by itself after this long.
    pub close_after: Option<Duration>,
    /// Use the dark or the light colours instead of Windows' setting.
    pub theme: Option<egui::Theme>,
}

impl DevOptions {
    /// Read `--screen home|editor`, `--screenshot <file.png>`, `--close-after <seconds>` and `--theme dark|light`.
    pub fn parse(args: &[String]) -> Result<DevOptions, String> {
        let mut options = DevOptions::default();
        let mut it = args.iter();
        while let Some(flag) = it.next() {
            let mut value = |what: &str| it.next().cloned().ok_or_else(|| format!("{flag} needs {what}."));
            match flag.as_str() {
                "--screen" => {
                    options.screen = Some(match value("home or editor")?.as_str() {
                        "home" => Screen::Home,
                        "editor" => Screen::Editor,
                        other => return Err(format!("--screen is home or editor, not {other:?}.")),
                    })
                }
                "--screenshot" => options.screenshot = Some(PathBuf::from(value("a file name ending in .png")?)),
                "--close-after" => {
                    let seconds: f32 = value("a number of seconds")?
                        .parse()
                        .map_err(|_| "--close-after needs a number of seconds.".to_string())?;
                    if !(0.0..=3600.0).contains(&seconds) {
                        return Err("--close-after is between 0 and 3600 seconds.".to_string());
                    }
                    options.close_after = Some(Duration::from_secs_f32(seconds));
                }
                "--theme" => {
                    options.theme = Some(match value("dark or light")?.as_str() {
                        "dark" => egui::Theme::Dark,
                        "light" => egui::Theme::Light,
                        other => return Err(format!("--theme is dark or light, not {other:?}.")),
                    })
                }
                other => return Err(format!("Unknown option {other:?}.")),
            }
        }
        Ok(options)
    }
}

struct ExpressApp {
    app: App,
    options: DevOptions,
    started: Instant,
    frames: u32,
    screenshot_requested: bool,
}

impl eframe::App for ExpressApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.draw(ui);
        self.dev_tick(ui.ctx());
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }
}

impl ExpressApp {
    /// The few things the `--screenshot` and `--close-after` switches need done between frames.
    fn dev_tick(&mut self, ctx: &egui::Context) {
        self.frames += 1;
        if let Some(path) = self.options.screenshot.clone() {
            if self.frames > 10 && !self.screenshot_requested {
                self.screenshot_requested = true;
                let context = ctx.clone();
                ctx.request_screenshot(move |image| {
                    if let Err(error) = save_png(&path, &image) {
                        eprintln!("The picture of the window could not be saved: {error}");
                    }
                    context.send_viewport_cmd(egui::ViewportCommand::Close);
                });
            }
            ctx.request_repaint();
        }
        if let Some(limit) = self.options.close_after {
            if self.started.elapsed() >= limit {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }
    }
}

fn save_png(path: &std::path::Path, image: &egui::ColorImage) -> Result<(), String> {
    let [width, height] = image.size;
    let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&bytes).map_err(|e| e.to_string())
}

/// Open the window and run until it is closed.
pub fn run(options: DevOptions) -> eframe::Result {
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("OpenStage Capture Express")
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "OpenStage Capture Express",
        native,
        Box::new(move |cc| {
            // The window is open, so the console box it may have started with is no longer needed. A run that takes a picture
            // of the window or closes it by itself is a developer's, and keeps its console to print errors in.
            #[cfg(all(windows, not(debug_assertions)))]
            if options.screenshot.is_none() && options.close_after.is_none() {
                openstage_express_windows::console::detach();
            }
            theme::install(&cc.egui_ctx);
            if let Some(forced) = options.theme {
                cc.egui_ctx.set_theme(forced);
            }
            let screen = options.screen.unwrap_or(Screen::Home);
            Ok(Box::new(ExpressApp {
                app: App::new(screen),
                options,
                started: Instant::now(),
                frames: 0,
                screenshot_requested: false,
            }))
        }),
    )
}
