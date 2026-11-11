// SPDX-License-Identifier: Apache-2.0
//! The window (milestones M5 and M6): Home, the capture bar and the Editor, drawn with egui and described to screen readers
//! through AccessKit, in the OpenStage suite's look (the navy bar, the menus, the command search: `shell.rs`, `menubar.rs`,
//! `commands.rs`).
//!
//! The screens (`home`, `bar`, `editor`) and their state (`editor_state`, `rows`) draw into a `Ui` they are given and know nothing
//! of the operating system, so the tests run whole screens without opening a window and read back what a screen reader would be
//! told. This file is the only place that opens a real window, switches it between the whole program and the strip of the
//! capture bar, listens for the hotkeys, and keeps Express's own windows out of its screenshots.

pub mod about;
pub mod app;
pub mod bar;
pub mod capture;
pub mod choose_area;
pub mod choose_window;
pub mod commands;
pub mod deleted;
pub mod editor;
pub mod editor_state;
pub mod export;
pub mod fonts;
pub mod home;
pub mod icon;
pub mod menubar;
pub mod open_copy;
pub mod rows;
pub mod settings;
pub mod shell;
pub mod theme;
pub mod trash;
// Made by OpenStage Author's tools/make_ui_icons.py (see the file's heading), so it is not formatted here.
#[rustfmt::skip]
#[allow(dead_code, clippy::approx_constant)]
pub mod uiicons_gen;
pub mod widgets;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use eframe::egui;

pub use app::{App, Screen, WindowMode};

use capture::HotkeyAction;

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
    /// Add one of each kind of mark to the example guide's first step, so a picture of the editor shows them.
    pub with_marks: bool,
    /// Open the window this many pixels tall, so that a picture of a long screen (Settings) shows more of it.
    pub height: Option<f32>,
    /// Keep the Settings page scrolled this many pixels down, so that a picture shows its lower part.
    pub scroll: Option<f32>,
    /// Choose the first three steps of the example guide, so a picture of the editor shows several chosen steps.
    pub choose_steps: bool,
    /// Delete two steps of the example guide, so a picture of the editor shows the Deleted steps button.
    pub delete_steps: bool,
    /// Open the question "Empty the trash for good?" on the Deleted steps screen.
    pub ask_to_empty: bool,
    /// Open this menu of the menu bar (its title), so a picture shows it.
    pub menu: Option<String>,
    /// Open the command search with these words typed, so a picture shows it.
    pub search: Option<String>,
    /// Open the list of keyboard keys, so a picture shows it.
    pub keys: bool,
}

impl DevOptions {
    /// Read `--screen home|editor|export|settings|about|bar`, `--screenshot <file.png>`, `--close-after <seconds>` and `--theme dark|light`.
    pub fn parse(args: &[String]) -> Result<DevOptions, String> {
        let mut options = DevOptions::default();
        let mut it = args.iter();
        while let Some(flag) = it.next() {
            let mut value = |what: &str| it.next().cloned().ok_or_else(|| format!("{flag} needs {what}."));
            match flag.as_str() {
                "--screen" => {
                    options.screen = Some(match value("home, editor, export, settings, about, trash, deleted, open-copy, bar, choose-window or choose-area")?.as_str() {
                        "home" => Screen::Home,
                        "editor" => Screen::Editor,
                        "export" => Screen::Export,
                        "settings" => Screen::Settings,
                        "about" => Screen::About,
                        "trash" => Screen::Trash,
                        "deleted" => Screen::DeletedSteps,
                        "open-copy" => Screen::OpenCopy,
                        "choose-window" => Screen::ChooseWindow,
                        "choose-area" => Screen::ChooseArea,
                        "bar" => Screen::Bar,
                        other => {
                            return Err(format!(
                                "--screen is home, editor, export, settings, about, trash, deleted, open-copy, bar, choose-window or choose-area, not {other:?}."
                            ))
                        }
                    })
                }
                "--with-marks" => options.with_marks = true,
                "--choose-steps" => options.choose_steps = true,
                "--delete-steps" => options.delete_steps = true,
                "--ask-to-empty" => options.ask_to_empty = true,
                "--keys" => options.keys = true,
                "--search" => options.search = Some(value("the words to search for")?),
                "--menu" => {
                    let title = value("the title of a menu")?;
                    if !commands::MENUS.iter().any(|m| m.title == title) {
                        return Err(format!("--menu is File, Edit, Step, Marks, View, Tools or Help, not {title:?}."));
                    }
                    options.menu = Some(title);
                }
                "--scroll" => {
                    let pixels: f32 = value("a number of pixels")?
                        .parse()
                        .map_err(|_| "--scroll needs a number of pixels.".to_string())?;
                    if !(0.0..=10_000.0).contains(&pixels) {
                        return Err("--scroll is between 0 and 10000 pixels.".to_string());
                    }
                    options.scroll = Some(pixels);
                }
                "--height" => {
                    let pixels: f32 = value("a number of pixels")?
                        .parse()
                        .map_err(|_| "--height needs a number of pixels.".to_string())?;
                    if !(400.0..=4000.0).contains(&pixels) {
                        return Err("--height is between 400 and 4000 pixels.".to_string());
                    }
                    options.height = Some(pixels);
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

/// What the hotkey listener tells the window.
enum HotkeyEvent {
    Pressed(HotkeyAction),
    /// The hotkeys were set, at the start and again after each change: the ones that work, and the ones that could not be set (for
    /// example because another program holds the key), each in words.
    Set {
        working: Vec<(HotkeyAction, openstage_express_core::hotkey::KeyCombo)>,
        problems: Vec<String>,
    },
}

/// The window's size as the whole program, and as the strip of the capture bar.
const NORMAL_SIZE: [f32; 2] = [1280.0, 760.0];
const NORMAL_MIN: [f32; 2] = [900.0, 560.0];
const BAR_SIZE: [f32; 2] = [620.0, 340.0];
const TALL_SIZE: [f32; 2] = [620.0, 620.0];
const BAR_MIN: [f32; 2] = [360.0, 150.0];

struct ExpressApp {
    app: App,
    options: DevOptions,
    started: Instant,
    frames: u32,
    screenshot_requested: bool,
    hotkeys: Option<Receiver<HotkeyEvent>>,
    /// Tells the hotkey listener the keys have been changed.
    hotkey_changes: Option<std::sync::mpsc::Sender<openstage_express_core::hotkey::HotkeySet>>,
    applied_mode: WindowMode,
    /// Where the whole-program window was before it became the bar, so that it goes back there.
    normal_position: Option<egui::Pos2>,
    #[cfg_attr(not(windows), allow(dead_code))]
    last_protected: Instant,
    /// The colour scheme last given to the window (so it is set only when it changes).
    applied_appearance: Option<crate::settings::Appearance>,
}

impl eframe::App for ExpressApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.take_hotkeys();
        self.app.draw(ui);
        self.apply_window_mode(ui.ctx());
        self.apply_appearance(ui.ctx());
        self.protect_own_windows();
        self.dev_tick(ui.ctx());
        if self.app.is_capturing() || self.app.is_exporting() {
            // The window may be in the background while someone works in another program: keep looking for what the capture
            // or the export has to say.
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }
}

impl ExpressApp {
    fn take_hotkeys(&mut self) {
        // New keys the person chose go to the listener, which lets go of the old ones and sets these.
        if let Some(keys) = self.app.take_hotkey_change() {
            if let Some(changes) = &self.hotkey_changes {
                let _ = changes.send(keys);
            }
        }
        let Some(events) = &self.hotkeys else { return };
        let events: Vec<HotkeyEvent> = events.try_iter().collect();
        for event in events {
            match event {
                HotkeyEvent::Pressed(action) => self.app.hotkey(action),
                HotkeyEvent::Set { working, problems } => {
                    self.app.set_hotkeys_working(&working);
                    if !problems.is_empty() {
                        self.app.status.say(problems.join(" "));
                    }
                }
            }
        }
    }

    /// Give the window the colour scheme the person chose, when it changes. A scheme forced on the command line wins.
    fn apply_appearance(&mut self, ctx: &egui::Context) {
        use crate::settings::Appearance;
        let chosen = self.app.appearance();
        if self.options.theme.is_some() || self.applied_appearance == Some(chosen) {
            return;
        }
        self.applied_appearance = Some(chosen);
        ctx.set_theme(match chosen {
            Appearance::System => egui::ThemePreference::System,
            Appearance::Light => egui::ThemePreference::Light,
            Appearance::Dark => egui::ThemePreference::Dark,
        });
    }

    /// Make the window the whole program or the strip of the capture bar, when that changes.
    fn apply_window_mode(&mut self, ctx: &egui::Context) {
        let mode = self.app.window_mode();
        if mode == self.applied_mode {
            return;
        }
        let before = self.applied_mode;
        self.applied_mode = mode;
        match mode {
            WindowMode::Bar | WindowMode::Tall => {
                let size = if mode == WindowMode::Tall { TALL_SIZE } else { BAR_SIZE };
                ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(BAR_MIN[0], BAR_MIN[1])));
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(size[0], size[1])));
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));
                // Coming from the whole program: remember where it was, and put the strip at the top right of the screen. Going
                // between the bar and the list of windows leaves the place alone.
                if before == WindowMode::Normal {
                    self.normal_position = ctx.input(|i| i.viewport().outer_rect).map(|r| r.min);
                    if let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) {
                        let left = (monitor.x - size[0] - 24.0).max(0.0);
                        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(left, 24.0)));
                    }
                }
            }
            WindowMode::Normal => {
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));
                ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(NORMAL_MIN[0], NORMAL_MIN[1])));
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(NORMAL_SIZE[0], NORMAL_SIZE[1])));
                if let Some(at) = self.normal_position {
                    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(at));
                }
            }
        }
    }
    /// Mark every window of this program as left out of pictures of the screen (spec 8.5). Windows made later are caught within
    /// half a second.
    fn protect_own_windows(&mut self) {
        #[cfg(windows)]
        if self.last_protected.elapsed() >= Duration::from_millis(500) {
            self.last_protected = Instant::now();
            openstage_express_windows::affinity::exclude_own_windows();
        }
    }

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

/// Listen for the hotkeys on a thread of their own and tell the window, waking it when one is pressed. The keys it listens for can be
/// changed by sending the new set. Not used by the runs that only take a picture of the window, so that they do not take the
/// hotkeys from a real copy of the program.
#[cfg(windows)]
fn listen_for_hotkeys(
    ctx: egui::Context,
    first: openstage_express_core::hotkey::HotkeySet,
) -> (Receiver<HotkeyEvent>, std::sync::mpsc::Sender<openstage_express_core::hotkey::HotkeySet>) {
    use openstage_express_windows::hotkeys::Hotkeys;
    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel();
    let (change_tx, change_rx) = mpsc::channel::<openstage_express_core::hotkey::HotkeySet>();
    std::thread::spawn(move || {
        let mut hotkeys = Some(Hotkeys::start(&first.entries()));
        let mut announce = true;
        loop {
            if let (true, Some(current)) = (announce, hotkeys.as_ref()) {
                announce = false;
                let set = HotkeyEvent::Set { working: current.working.clone(), problems: current.problems.clone() };
                if tx.send(set).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
            // The person chose other keys. Let go of the old ones first: setting a key that is already set by this same program
            // would be refused.
            let newest = change_rx.try_iter().last();
            if let Some(keys) = newest {
                hotkeys.take();
                hotkeys = Some(Hotkeys::start(&keys.entries()));
                announce = true;
                continue;
            }
            let Some(current) = hotkeys.as_ref() else { break };
            match current.next(Duration::from_millis(200)) {
                Ok(action) => {
                    if tx.send(HotkeyEvent::Pressed(action)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
    (rx, change_tx)
}

/// The program's icon for the title bar and the task bar, from the icon pack (256 pixels).
pub fn window_icon() -> Option<egui::IconData> {
    const ICON: &[u8] = include_bytes!("../../../../assets/icon/PNG/OpenCapture_Express_256.png");
    let mut decoder = png::Decoder::new(std::io::Cursor::new(ICON));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).ok()?;
    if info.color_type != png::ColorType::Rgba {
        return None;
    }
    buffer.truncate(info.buffer_size());
    Some(egui::IconData { rgba: buffer, width: info.width, height: info.height })
}

/// Open the window and run until it is closed.
pub fn run(options: DevOptions) -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("OpenStage Capture Express")
        .with_inner_size([NORMAL_SIZE[0], options.height.unwrap_or(NORMAL_SIZE[1])])
        .with_min_inner_size(NORMAL_MIN);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    let native = eframe::NativeOptions { viewport, ..Default::default() };
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
            let with_marks = options.with_marks;
            let settings_path = crate::settings::default_path();
            let (settings, settings_note) = match &settings_path {
                // A run that takes a picture of the window must not depend on, or change, the person's settings.
                Some(path) if options.screenshot.is_none() && options.close_after.is_none() => {
                    crate::settings::load(path)
                }
                _ => (crate::settings::AppSettings::default(), None),
            };
            let settings_path = settings_path.filter(|_| options.screenshot.is_none() && options.close_after.is_none());
            let library = crate::library::guides_folder(settings.guides_path());
            let exports = crate::library::exports_folder(settings.exports_path(), &library);
            #[cfg(windows)]
            let (app, hotkeys, hotkey_changes) = {
                let dev_run = options.screenshot.is_some() || options.close_after.is_some();
                let (hotkeys, hotkey_changes) =
                    match (!dev_run).then(|| listen_for_hotkeys(cc.egui_ctx.clone(), settings.hotkeys)) {
                        Some((events, changes)) => (Some(events), Some(changes)),
                        None => (None, None),
                    };
                // Until the listener says which keys it could set, say the ones the person chose.
                let words = openstage_express_core::hotkey::sentence(&settings.hotkeys.entries());
                let mut app = App::new(screen)
                    .with_capture(
                        library,
                        crate::capture_win::starter(),
                        &crate::capture_win::ready_words(),
                        Box::new(openstage_express_windows::system::process_is_running),
                    )
                    .with_desktop(Box::new(crate::capture_win::WindowsDesktop))
                    .with_cues(Box::new(crate::capture_win::WindowsCues))
                    .with_system(
                        exports,
                        crate::export_run::system_pdf().is_some(),
                        Box::new(|start| {
                            openstage_express_windows::dialogs::choose_folder(start, "Choose where to put the exports")
                        }),
                        Box::new(|folder| {
                            let _ = std::process::Command::new("explorer").arg(folder).spawn();
                        }),
                    )
                    .with_file_chooser(Box::new(|start| {
                        openstage_express_windows::dialogs::choose_file(
                            start,
                            "Choose a copy of a guide",
                            "Copies of guides (*.zip)",
                            "*.zip",
                        )
                    }));
                app.set_hotkey_words(&words);
                (app.with_settings(settings, settings_path, settings_note), hotkeys, hotkey_changes)
            };
            #[cfg(not(windows))]
            let (app, hotkeys, hotkey_changes) = (
                App::new(screen).with_capture(
                    library,
                    capture::cannot_capture(),
                    "the whole screen",
                    Box::new(|_| false),
                ),
                None,
                None,
            );
            let mut app = app;
            if let Some(y) = options.scroll {
                app = app.with_settings_scroll(y);
            }
            if with_marks {
                app.add_demo_marks();
            }
            if options.delete_steps && screen != Screen::DeletedSteps {
                app.delete_demo_steps();
            }
            if options.ask_to_empty {
                app.ask_to_empty_demo();
            }
            if options.choose_steps {
                app.choose_demo_steps();
            }
            if let Some(words) = &options.search {
                app.search = Some(shell::SearchState { typed: words.clone(), chosen: 0, focus_next: true });
            }
            app.keys_open = options.keys;
            if let Some(title) = &options.menu {
                if let Some(at) = commands::MENUS.iter().position(|m| m.title == title.as_str()) {
                    menubar::open_at_start(&cc.egui_ctx, at);
                }
            }
            // The two choosing screens are looked at over a made-up desktop, so no one's real windows are shown.
            match screen {
                Screen::ChooseWindow => {
                    app = app.with_desktop(Box::new(capture::DemoDesktop));
                    app.show_choose_window();
                }
                Screen::ChooseArea => {
                    app = app.with_desktop(Box::new(capture::DemoDesktop));
                    app.show_choose_area();
                }
                _ => {}
            }
            Ok(Box::new(ExpressApp {
                app,
                options,
                started: Instant::now(),
                frames: 0,
                screenshot_requested: false,
                hotkeys,
                hotkey_changes,
                applied_mode: WindowMode::Normal,
                normal_position: None,
                last_protected: Instant::now(),
                applied_appearance: None,
            }))
        }),
    )
}
