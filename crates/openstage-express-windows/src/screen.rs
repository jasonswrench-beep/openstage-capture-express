// SPDX-License-Identifier: Apache-2.0
//! The pictures of the screen (spec section 8.5): Windows Graphics Capture of one display, through OpenStage Capture's
//! `Platform` and `FrameSource`. Windows leaves the cursor out of these pictures, which suits Express: it draws its own cursor
//! as a mark the author can change.
//!
//! The capture runs on its own thread and hands pictures over a small queue; if the program is too slow to take them, pictures
//! are thrown away rather than piling up in memory. The display is captured whole, even in window mode: menus and tooltips are
//! windows of their own and would be missing from a capture of one window (spec K8), so a window is cropped out afterwards.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use openstage_capture_windows::{system_platform, DisplayInfo};
use openstage_express_core::capture::{Area, Picture};
use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTONEAREST};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

/// The displays, the main one first (Windows' own numbering for Express: 1 is the main display).
pub fn displays() -> Result<Vec<DisplayInfo>, String> {
    system_platform().list_displays().map_err(|e| e.to_string())
}

/// Which Windows this is, in words, and its build number.
pub fn windows_version() -> Result<(String, u32), String> {
    let version = system_platform().windows_version().map_err(|e| e.to_string())?;
    Ok((version.describe(), version.build))
}

/// Whether Windows' screen capture service is available.
pub fn capture_supported() -> bool {
    system_platform().capture_supported()
}

/// The part of the desktop a display covers.
pub fn area_of(display: &DisplayInfo) -> Area {
    Area { x: display.x, y: display.y, width: display.width, height: display.height }
}

/// The display's scaling in percent (100, 125, 150, ...), so the marks look the same size on any display.
pub fn dpi_percent(display: &DisplayInfo) -> u32 {
    let centre = POINT {
        x: display.x.saturating_add((display.width / 2) as i32),
        y: display.y.saturating_add((display.height / 2) as i32),
    };
    unsafe {
        let monitor = MonitorFromPoint(centre, MONITOR_DEFAULTTONEAREST);
        let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
        match GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) {
            Ok(()) if dpi_x > 0 => ((dpi_x as u64 * 100 + 48) / 96) as u32,
            _ => 100,
        }
    }
}

/// The display with this number (1 is the main display), or the main display if there is no such number.
pub fn pick_display(all: &[DisplayInfo], number: u32) -> Option<&DisplayInfo> {
    all.iter().find(|d| d.number == number).or_else(|| all.iter().find(|d| d.main)).or_else(|| all.first())
}

/// Pictures of one display, arriving as they are taken.
pub struct ScreenFeed {
    frames: Receiver<(Instant, Picture)>,
    stop: Arc<AtomicBool>,
    skipped: Arc<AtomicU64>,
    ended: Arc<Mutex<Option<String>>>,
    thread: Option<JoinHandle<()>>,
    display: DisplayInfo,
}

/// How many pictures may wait to be taken by the noting thread. A 1080p picture is about 8 megabytes.
const WAITING: usize = 3;

impl ScreenFeed {
    /// Start capturing display number `number`. A picture closer than `min_gap` to the one before is dropped: the ring only
    /// needs a few a second, and each one costs a read-back from the graphics card.
    pub fn start(number: u32, min_gap: Duration) -> Result<ScreenFeed, String> {
        let (tx, frames) = mpsc::sync_channel::<(Instant, Picture)>(WAITING);
        let stop = Arc::new(AtomicBool::new(false));
        let skipped = Arc::new(AtomicU64::new(0));
        let ended = Arc::new(Mutex::new(None));
        let (ready_tx, ready_rx) = mpsc::channel::<Result<DisplayInfo, String>>();
        let thread = {
            let (stop, skipped, ended) = (Arc::clone(&stop), Arc::clone(&skipped), Arc::clone(&ended));
            std::thread::spawn(move || {
                // The capture belongs to this thread: it is made here and never leaves.
                let platform = system_platform();
                let all = match platform.list_displays() {
                    Ok(all) if !all.is_empty() => all,
                    Ok(_) => {
                        let _ = ready_tx.send(Err("Windows reports no display to capture.".to_string()));
                        return;
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e.to_string()));
                        return;
                    }
                };
                let Some(display) = pick_display(&all, number).cloned() else {
                    let _ = ready_tx.send(Err("There is no such display.".to_string()));
                    return;
                };
                let mut source = match platform.start_display_capture(&display, false) {
                    Ok(source) => source,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e.to_string()));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(display));
                let mut last_kept: Option<Instant> = None;
                while !stop.load(Ordering::Acquire) {
                    match source.next_frame(Duration::from_millis(100)) {
                        Some(frame) => {
                            let now = Instant::now();
                            if last_kept.is_some_and(|t| now.duration_since(t) < min_gap) {
                                source.recycle(frame);
                                continue;
                            }
                            let Some(picture) = Picture::new(frame.width, frame.height, frame.bgra) else { continue };
                            last_kept = Some(now);
                            match tx.try_send((now, picture)) {
                                Ok(()) => {}
                                Err(TrySendError::Full(_)) => {
                                    skipped.fetch_add(1, Ordering::Relaxed);
                                }
                                Err(TrySendError::Disconnected(_)) => break,
                            }
                        }
                        None => {
                            if let Some(why) = source.ended() {
                                *ended.lock().unwrap_or_else(|e| e.into_inner()) = Some(why);
                                break;
                            }
                        }
                    }
                }
            })
        };
        match ready_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(display)) => Ok(ScreenFeed { frames, stop, skipped, ended, thread: Some(thread), display }),
            Ok(Err(why)) => {
                let _ = thread.join();
                Err(why)
            }
            Err(_) => {
                stop.store(true, Ordering::Release);
                Err("Windows did not start the screen capture in time.".to_string())
            }
        }
    }

    pub fn display(&self) -> &DisplayInfo {
        &self.display
    }

    /// The next picture, waiting up to `wait` for one.
    pub fn next(&self, wait: Duration) -> Result<(Instant, Picture), RecvTimeoutError> {
        self.frames.recv_timeout(wait)
    }

    /// Pictures thrown away because the program was too slow to take them.
    pub fn skipped(&self) -> u64 {
        self.skipped.load(Ordering::Relaxed)
    }

    /// Why the capture stopped by itself (the display went away), if it did.
    pub fn ended(&self) -> Option<String> {
        self.ended.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl Drop for ScreenFeed {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            // The thread notices within a tenth of a second. Wait a little, but never hang the program on it.
            for _ in 0..50 {
                if thread.is_finished() {
                    let _ = thread.join();
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(number: u32, main: bool) -> DisplayInfo {
        // DisplayInfo cannot be built outside its crate, so the tests use the real list.
        let all = displays().expect("this PC has a display");
        let mut chosen = all.into_iter().next().expect("at least one display");
        chosen.number = number;
        chosen.main = main;
        chosen
    }

    #[test]
    fn the_area_of_a_display_is_where_it_sits_on_the_desktop() {
        let d = display(1, true);
        let area = area_of(&d);
        assert_eq!((area.x, area.y, area.width, area.height), (d.x, d.y, d.width, d.height));
    }

    #[test]
    fn a_display_number_that_does_not_exist_falls_back_to_the_main_display() {
        let (one, two) = (display(1, true), display(2, false));
        let all = vec![two.clone(), one.clone()];
        assert_eq!(pick_display(&all, 2).map(|d| d.number), Some(2));
        assert_eq!(pick_display(&all, 9).map(|d| d.number), Some(1));
        assert!(pick_display(&[], 1).is_none());
    }

    #[test]
    fn the_display_scaling_is_a_sensible_percentage() {
        let d = display(1, true);
        let percent = dpi_percent(&d);
        assert!((100..=500).contains(&percent), "{percent}");
    }
}
