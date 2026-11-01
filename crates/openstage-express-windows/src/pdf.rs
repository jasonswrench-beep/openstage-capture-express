// SPDX-License-Identifier: Apache-2.0
//! PDF files from the Microsoft WebView2 engine (spec sections 6.3 and 10.2).
//!
//! WebView2 is the web engine that Windows 11 ships with (Windows 10 has it whenever Microsoft Edge or Microsoft 365 is
//! installed). It is asked to open the print page that `openstage_express_export::pdf` wrote and to print it with the
//! DevTools command `Page.printToPDF`, because that command, unlike the plain `PrintToPdf` method, can write a *tagged*
//! PDF (headings, lists, links and pictures' alt text) and bookmarks.
//!
//! Nothing here uses the network. The page is served to the engine from a folder of our own under a made-up host name, with
//! scripts switched off; a security policy in the page itself forbids everything but its own style and pictures; and the
//! engine is started with its background networking, updates and reputation checks turned off. Its working folder is made
//! in the temporary folder for each export and removed afterwards.
//!
//! The engine needs a window and a message loop, so each export runs on its own thread, with a hidden window that is never
//! shown. A stuck engine is given up on after a time limit rather than waited for forever.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use base64::Engine as _;
use openstage_express_export::pdf::{PdfRenderer, PrintPage, RenderError};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::{
    CallDevToolsProtocolMethodCompletedHandler, CoTaskMemPWSTR, CoreWebView2EnvironmentOptions,
    CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    NavigationCompletedEventHandler,
};
use windows::core::{w, Interface, BOOL, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{E_POINTER, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, MsgWaitForMultipleObjects, PeekMessageW,
    RegisterClassW, TranslateMessage, CW_USEDEFAULT, MSG, PM_REMOVE, QS_ALLINPUT, WNDCLASSW, WS_OVERLAPPED,
};

/// The made-up host name the print page is served under. It is never looked up on a network.
const HOST: &str = "express-print.test";
const PAGE_FILE: &str = "print.html";
const NOT_INSTALLED: &str =
    "PDF export needs the Microsoft Edge WebView2 Runtime, and it is not installed on this computer. \
It comes with Windows 11 and with Microsoft Edge. Install it from Microsoft, or export the web page instead.";

const START_TIMEOUT: Duration = Duration::from_secs(90);
const WINDOW_TIMEOUT: Duration = Duration::from_secs(60);
const LOAD_TIMEOUT: Duration = Duration::from_secs(60);
const PRINT_TIMEOUT: Duration = Duration::from_secs(180);
const OVERALL_TIMEOUT: Duration = Duration::from_secs(420);

/// Options for the engine's own start-up: no background networking, no update or reputation checks, no sync.
const ENGINE_ARGUMENTS: &str =
    "--disable-background-networking --disable-component-update --disable-domain-reliability \
--disable-sync --no-pings --disable-features=msSmartScreenProtection";

/// The DevTools command's settings: use the page's own `@page` paper and margins, write tags and bookmarks, no browser
/// header or footer (the page draws its own page numbers), no backgrounds.
const PRINT_PARAMETERS: &str =
    "{\"preferCSSPageSize\":true,\"generateTaggedPDF\":true,\"generateDocumentOutline\":true,\
\"displayHeaderFooter\":false,\"printBackground\":false,\"transferMode\":\"ReturnAsBase64\"}";

/// Makes PDF files with the WebView2 engine.
#[derive(Debug, Default, Clone, Copy)]
pub struct WebView2Pdf;

impl WebView2Pdf {
    /// The version of the WebView2 runtime on this computer, or `None` if there is none.
    pub fn runtime_version() -> Option<String> {
        let mut version = PWSTR::null();
        // SAFETY: the loader fills `version` with a string it allocated, which CoTaskMemPWSTR takes and frees.
        unsafe {
            webview2_com::Microsoft::Web::WebView2::Win32::GetAvailableCoreWebView2BrowserVersionString(
                PCWSTR::null(),
                &mut version,
            )
            .ok()?;
        }
        if version.is_null() {
            return None;
        }
        let text = CoTaskMemPWSTR::from(version).to_string();
        (!text.trim().is_empty()).then_some(text)
    }
}

impl PdfRenderer for WebView2Pdf {
    fn render(&self, page: &PrintPage<'_>) -> Result<Vec<u8>, RenderError> {
        if Self::runtime_version().is_none() {
            return Err(RenderError::Unavailable(NOT_INSTALLED.to_string()));
        }
        let html = page.html.to_string();
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("webview2-pdf".to_string())
            .spawn(move || {
                let _ = tx.send(run_job(&html));
            })
            .map_err(|e| RenderError::Failed(format!("The PDF engine could not be started: {e}.")))?;
        match rx.recv_timeout(OVERALL_TIMEOUT) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => Err(RenderError::Failed(
                "The PDF engine did not finish in time, so it was given up on. A guide with very many pictures may need \
a smaller picture width or fewer steps."
                    .to_string(),
            )),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(RenderError::Failed("The PDF engine stopped without giving back a file.".to_string()))
            }
        }
    }
}

/// Runs one export on this (new) thread: a private working folder, the engine, and the clean-up.
fn run_job(html: &str) -> Result<Vec<u8>, RenderError> {
    let work = unique_work_folder();
    let site = work.join("site");
    fs::create_dir_all(&site)
        .and_then(|()| fs::write(site.join(PAGE_FILE), html))
        .map_err(|e| RenderError::Failed(format!("The PDF engine's working folder could not be made: {e}.")))?;

    // SAFETY: balanced with CoUninitialize below; nothing COM-related outlives `print_in_engine`.
    let initialised = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if let Err(e) = initialised.ok() {
        remove_folder(&work);
        return Err(RenderError::Failed(format!("Windows would not let the PDF engine start: {e}.")));
    }
    let result = print_in_engine(&work);
    // SAFETY: pairs with the successful CoInitializeEx above.
    unsafe { CoUninitialize() };
    remove_folder(&work);
    result
}

fn unique_work_folder() -> PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    std::env::temp_dir().join(format!("openstage-express-pdf-{}-{nanos}", std::process::id()))
}

/// Removes our own working folder. The engine lets go of its profile a moment after it is closed, so try for a few seconds;
/// if it still cannot be removed the operating system's temporary-file clean-up gets it.
fn remove_folder(folder: &Path) {
    for _ in 0..20 {
        if !folder.exists() || fs::remove_dir_all(folder).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn failed(stage: &str) -> impl FnOnce(windows::core::Error) -> RenderError + '_ {
    move |e| RenderError::Failed(format!("The PDF engine stopped while {stage}: {e}."))
}

/// A window that is never shown. The engine needs one to belong to.
struct HiddenWindow(HWND);

unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: the default handler is always safe to call with the arguments Windows gave us.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

impl HiddenWindow {
    fn new() -> windows::core::Result<HiddenWindow> {
        // SAFETY: plain window-class and window creation with valid, static arguments.
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: HINSTANCE(instance.0),
                lpszClassName: w!("OpenStageExpressPdf"),
                ..Default::default()
            };
            // Registering a second time fails harmlessly; the class is already there.
            RegisterClassW(&class);
            let hwnd = CreateWindowExW(
                Default::default(),
                w!("OpenStageExpressPdf"),
                w!("OpenStage Capture Express PDF engine"),
                WS_OVERLAPPED,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                1024,
                768,
                None,
                None,
                Some(HINSTANCE(instance.0)),
                None,
            )?;
            Ok(HiddenWindow(hwnd))
        }
    }
}

impl Drop for HiddenWindow {
    fn drop(&mut self) {
        // SAFETY: the window was made by this thread and is destroyed once.
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}

/// Closes the engine's view when it goes out of scope, before the environment and the window.
struct Controller(ICoreWebView2Controller);

impl Drop for Controller {
    fn drop(&mut self) {
        // SAFETY: Close is the documented way to finish with a controller.
        unsafe {
            let _ = self.0.Close();
        }
    }
}

/// Keeps handling this thread's window messages (which is how the engine reports back) until `rx` has something to say or
/// `timeout` passes.
fn pump_until<T>(rx: &mpsc::Receiver<T>, timeout: Duration) -> Option<T> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(value) = rx.try_recv() {
            return Some(value);
        }
        let now = Instant::now();
        if now >= deadline {
            return None;
        }
        let wait = (deadline - now).min(Duration::from_millis(100)).as_millis() as u32;
        // SAFETY: an ordinary message loop on the thread that owns the windows.
        unsafe {
            MsgWaitForMultipleObjects(None, false, wait, QS_ALLINPUT);
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}

fn timed_out(stage: &str) -> RenderError {
    RenderError::Failed(format!("The PDF engine did not answer in time while {stage}."))
}

fn print_in_engine(work: &Path) -> Result<Vec<u8>, RenderError> {
    let window = HiddenWindow::new().map_err(failed("opening its window"))?;

    // The engine's environment, with its own private profile folder.
    let (tx, rx) = mpsc::channel();
    let options = CoreWebView2EnvironmentOptions::default();
    // SAFETY: setting a plain string before the options object is handed over.
    unsafe { options.set_additional_browser_arguments(ENGINE_ARGUMENTS.to_string()) };
    let profile = HSTRING::from(work.join("profile").to_string_lossy().as_ref());
    // SAFETY: valid arguments; the handler runs on this thread while we pump messages below.
    unsafe {
        CreateCoreWebView2EnvironmentWithOptions(
            PCWSTR::null(),
            &profile,
            &ICoreWebView2EnvironmentOptions::from(options),
            &CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(move |error_code, environment| {
                let result = error_code.and_then(|()| environment.ok_or_else(|| windows::core::Error::from(E_POINTER)));
                let _ = tx.send(result);
                Ok(())
            })),
        )
        .map_err(failed("starting"))?;
    }
    let environment =
        pump_until(&rx, START_TIMEOUT).ok_or_else(|| timed_out("starting"))?.map_err(failed("starting"))?;

    // The view that will hold the page.
    let (tx, rx) = mpsc::channel();
    // SAFETY: as above.
    unsafe {
        environment
            .CreateCoreWebView2Controller(
                window.0,
                &CreateCoreWebView2ControllerCompletedHandler::create(Box::new(move |error_code, controller| {
                    let result =
                        error_code.and_then(|()| controller.ok_or_else(|| windows::core::Error::from(E_POINTER)));
                    let _ = tx.send(result);
                    Ok(())
                })),
            )
            .map_err(failed("opening a view"))?;
    }
    let controller = Controller(
        pump_until(&rx, WINDOW_TIMEOUT)
            .ok_or_else(|| timed_out("opening a view"))?
            .map_err(failed("opening a view"))?,
    );
    // SAFETY: plain property calls on live objects.
    let webview = unsafe {
        controller
            .0
            .SetBounds(RECT { left: 0, top: 0, right: 1024, bottom: 768 })
            .map_err(failed("sizing its view"))?;
        controller.0.CoreWebView2().map_err(failed("opening a view"))?
    };

    // No scripts, no menus, no developer tools, and the page comes from our own folder under a made-up host name.
    // SAFETY: as above.
    unsafe {
        let settings = webview.Settings().map_err(failed("setting up"))?;
        settings.SetIsScriptEnabled(false).map_err(failed("setting up"))?;
        settings.SetAreDefaultContextMenusEnabled(false).map_err(failed("setting up"))?;
        settings.SetAreDevToolsEnabled(false).map_err(failed("setting up"))?;
        let webview3: ICoreWebView2_3 = webview.cast().map_err(failed("setting up"))?;
        webview3
            .SetVirtualHostNameToFolderMapping(
                &HSTRING::from(HOST),
                &HSTRING::from(work.join("site").to_string_lossy().as_ref()),
                COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_DENY,
            )
            .map_err(failed("setting up"))?;
    }

    // Load the page and wait until it has finished loading.
    let (tx, rx) = mpsc::channel();
    let mut token = 0i64;
    // SAFETY: the handler is removed again before the view goes away.
    unsafe {
        webview
            .add_NavigationCompleted(
                &NavigationCompletedEventHandler::create(Box::new(move |_sender, args| {
                    let mut success = BOOL(0);
                    if let Some(args) = args {
                        args.IsSuccess(&mut success)?;
                    }
                    let _ = tx.send(success.as_bool());
                    Ok(())
                })),
                &mut token,
            )
            .map_err(failed("loading the page"))?;
        webview.Navigate(&HSTRING::from(format!("https://{HOST}/{PAGE_FILE}"))).map_err(failed("loading the page"))?;
    }
    let loaded = pump_until(&rx, LOAD_TIMEOUT).ok_or_else(|| timed_out("loading the page"))?;
    // SAFETY: removes the handler added above.
    unsafe {
        let _ = webview.remove_NavigationCompleted(token);
    }
    if !loaded {
        return Err(RenderError::Failed("The PDF engine could not load the print page.".to_string()));
    }

    // Print it.
    let (tx, rx) = mpsc::channel();
    // SAFETY: as above.
    unsafe {
        webview
            .CallDevToolsProtocolMethod(
                &HSTRING::from("Page.printToPDF"),
                &HSTRING::from(PRINT_PARAMETERS),
                &CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |error_code, json: String| {
                    let _ = tx.send(error_code.map(|()| json));
                    Ok(())
                })),
            )
            .map_err(failed("printing"))?;
    }
    let reply = pump_until(&rx, PRINT_TIMEOUT).ok_or_else(|| timed_out("printing"))?.map_err(failed("printing"))?;
    pdf_from_reply(&reply)
}

/// The PDF inside the engine's JSON reply (`{"data":"<base64>"}`).
fn pdf_from_reply(reply: &str) -> Result<Vec<u8>, RenderError> {
    let value: serde_json::Value = serde_json::from_str(reply)
        .map_err(|_| RenderError::Failed("The PDF engine's answer could not be read.".to_string()))?;
    let data = value
        .get("data")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| RenderError::Failed("The PDF engine did not give back a file.".to_string()))?;
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|_| RenderError::Failed("The PDF engine's file could not be read.".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstage_capture_render::BlockText;
    use openstage_express_export::demo::demo;
    use openstage_express_export::pdf::{audit_pdf, export_pdf, print_page, Orientation, PageSize, PdfOptions};
    use openstage_express_export::plan::{plan, ExportOptions, Plan};

    fn demo_plan() -> Plan {
        let d = demo();
        plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default())
    }

    /// The tests that use the real engine take turns, because one of them counts the working folders the others make.
    static ENGINE_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Some(a turn at the engine) when there is a WebView2 runtime to test with. The tests that need one say so loudly when
    /// they skip.
    fn engine_present(test: &str) -> Option<std::sync::MutexGuard<'static, ()>> {
        match WebView2Pdf::runtime_version() {
            Some(_) => Some(ENGINE_TESTS.lock().unwrap_or_else(|poisoned| poisoned.into_inner())),
            None => {
                eprintln!("SKIPPED {test}: no WebView2 runtime on this computer, so nothing was tested.");
                None
            }
        }
    }

    /// The first page's width and height in points, read from the plain `/MediaBox [ 0 0 w h ]` the engine writes.
    fn first_media_box(pdf: &[u8]) -> Option<(f64, f64)> {
        let text = String::from_utf8_lossy(pdf);
        let at = text.find("/MediaBox")?;
        let rest = &text[at + "/MediaBox".len()..];
        let open = rest.find('[')?;
        let close = rest.find(']')?;
        let numbers: Vec<f64> = rest[open + 1..close].split_whitespace().filter_map(|n| n.parse().ok()).collect();
        (numbers.len() == 4).then(|| (numbers[2] - numbers[0], numbers[3] - numbers[1]))
    }

    #[test]
    fn the_engines_reply_is_unpacked_and_bad_replies_are_refused_in_plain_words() {
        let reply = format!("{{\"data\":\"{}\"}}", base64::engine::general_purpose::STANDARD.encode(b"%PDF-1.4 hello"));
        assert_eq!(pdf_from_reply(&reply).unwrap(), b"%PDF-1.4 hello");
        for bad in ["", "not json", "{}", "{\"data\":5}", "{\"data\":\"***\"}"] {
            let error = pdf_from_reply(bad).unwrap_err();
            assert!(matches!(error, RenderError::Failed(_)), "{bad:?}");
            assert!(error.to_string().starts_with("The PDF engine"), "{error}");
        }
    }

    #[test]
    fn the_work_folders_are_unique_and_in_the_temporary_folder() {
        let a = unique_work_folder();
        std::thread::sleep(Duration::from_millis(2));
        let b = unique_work_folder();
        assert_ne!(a, b);
        assert!(a.starts_with(std::env::temp_dir()));
        assert!(a.file_name().unwrap().to_string_lossy().starts_with("openstage-express-pdf-"));
    }

    #[test]
    fn the_print_settings_ask_for_tags_bookmarks_and_the_pages_own_paper() {
        let settings: serde_json::Value = serde_json::from_str(PRINT_PARAMETERS).unwrap();
        assert_eq!(settings["generateTaggedPDF"], true);
        assert_eq!(settings["generateDocumentOutline"], true);
        assert_eq!(settings["preferCSSPageSize"], true);
        assert_eq!(settings["displayHeaderFooter"], false);
    }

    #[test]
    fn the_runtime_is_found_on_this_computer() {
        let Some(_engine) = engine_present("the_runtime_is_found_on_this_computer") else {
            return;
        };
        let version = WebView2Pdf::runtime_version().unwrap();
        assert!(version.chars().next().unwrap().is_ascii_digit(), "{version}");
        assert!(version.contains('.'), "{version}");
    }

    #[test]
    fn the_demo_guide_becomes_a_tagged_pdf_with_bookmarks_and_alt_text() {
        let Some(_engine) = engine_present("the_demo_guide_becomes_a_tagged_pdf_with_bookmarks_and_alt_text") else {
            return;
        };
        let plan = demo_plan();
        let options = PdfOptions::default();
        let export = export_pdf(&plan, &options, &WebView2Pdf).expect("the PDF should be made");
        let bytes = &export.files[0].bytes;
        let audit = audit_pdf(bytes);
        let pictures = plan.steps.iter().filter(|s| s.picture.is_some()).count();
        assert!(audit.is_pdf && audit.tagged && audit.language && audit.bookmarks, "{audit:?}");
        assert!(audit.pages >= 2, "{audit:?}");
        let steps = plan.steps.iter().filter(|s| s.kind != openstage_express_core::StepKind::Block).count();
        assert!(audit.headings > steps, "the title, the contents and every step are headings: {audit:?}");
        assert!(audit.alt_texts >= pictures, "every picture carries its description: {audit:?}");
        assert!(audit.title_shown, "{audit:?}");
        assert_eq!(export.files[0].path, "How to submit an assignment.pdf");
    }

    #[test]
    fn the_paper_and_orientation_in_the_options_are_the_paper_in_the_file() {
        let Some(_engine) = engine_present("the_paper_and_orientation_in_the_options_are_the_paper_in_the_file") else {
            return;
        };
        let plan = demo_plan();
        for (size, orientation, width, height) in [
            (PageSize::A4, Orientation::Portrait, 595.3, 841.9),
            (PageSize::Letter, Orientation::Landscape, 792.0, 612.0),
        ] {
            let options = PdfOptions { page_size: size, orientation, ..PdfOptions::default() };
            let pdf = WebView2Pdf
                .render(&PrintPage { html: &print_page(&plan, &options), title: "t", language: "en" })
                .unwrap();
            let (w, h) = first_media_box(&pdf).expect("a media box");
            assert!((w - width).abs() < 2.0 && (h - height).abs() < 2.0, "{size:?} {orientation:?}: {w} x {h} points");
        }
    }

    /// Every `/Title` string in the file, in the order they appear: the document's own title and then each bookmark. Reads
    /// the plain `( ... )` strings and the `<FEFF ... >` hexadecimal ones the engine writes.
    fn titles(pdf: &[u8]) -> Vec<String> {
        let text: String = pdf.iter().map(|&b| b as char).collect();
        let mut found = Vec::new();
        for (at, _) in text.match_indices("/Title") {
            let rest = text[at + "/Title".len()..].trim_start();
            if let Some(body) = rest.strip_prefix('(') {
                let mut bytes = Vec::new();
                let mut chars = body.chars();
                while let Some(c) = chars.next() {
                    match c {
                        ')' => break,
                        '\\' => match chars.next() {
                            Some('n') => bytes.push(b'\n'),
                            Some(c @ ('(' | ')' | '\\')) => bytes.push(c as u8),
                            Some(d) if d.is_digit(8) => {
                                let mut value = d.to_digit(8).unwrap();
                                for _ in 0..2 {
                                    let mut peek = chars.clone();
                                    match peek.next() {
                                        Some(n) if n.is_digit(8) => {
                                            value = value * 8 + n.to_digit(8).unwrap();
                                            chars = peek;
                                        }
                                        _ => break,
                                    }
                                }
                                bytes.push(value as u8);
                            }
                            Some(other) => bytes.push(other as u8),
                            None => break,
                        },
                        c => bytes.push(c as u8),
                    }
                }
                found.push(decode_pdf_text(&bytes));
            } else if let Some(body) = rest.strip_prefix('<') {
                let hex: String = body.chars().take_while(|c| *c != '>').filter(|c| c.is_ascii_hexdigit()).collect();
                let bytes: Vec<u8> =
                    (0..hex.len() / 2).filter_map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()).collect();
                found.push(decode_pdf_text(&bytes));
            }
        }
        found
    }

    /// PDF text: UTF-16 with a byte order mark, or else plain bytes.
    fn decode_pdf_text(bytes: &[u8]) -> String {
        if bytes.starts_with(&[0xFE, 0xFF]) {
            let (pairs, _) = bytes[2..].as_chunks::<2>();
            let units: Vec<u16> = pairs.iter().map(|p| u16::from_be_bytes(*p)).collect();
            String::from_utf16_lossy(&units)
        } else {
            bytes.iter().map(|&b| b as char).collect()
        }
    }

    /// What is wrong with a PDF's bookmarks: every wanted title must be there exactly, and nothing else may be made of a
    /// wanted title's words. WebView2 once wrote some headings' bookmarks twice, as `2.2. Click "Save" Click "Save"`.
    fn bookmark_problems(found: &[String], wanted: &[&str]) -> Vec<String> {
        let mut problems = Vec::new();
        for want in wanted {
            if !found.iter().any(|t| t == want) {
                problems.push(format!("no bookmark {want:?}"));
            }
        }
        for title in found {
            if wanted.contains(&title.as_str()) {
                continue;
            }
            let repeats = wanted.iter().any(|w| {
                let words = w.split_once(' ').map_or(*w, |(_, rest)| rest);
                title.contains(words)
            });
            if repeats {
                problems.push(format!("a bookmark made of a title's words again: {title:?}"));
            }
        }
        problems
    }

    #[test]
    fn the_bookmark_check_accepts_good_titles_and_refuses_the_doubled_ones_the_engine_once_wrote() {
        let wanted = ["Guide", "1. Click \"A\"", "2. Click \"New assignment\"", "3. Select \"Course\"", "Contents"];
        let good: Vec<String> =
            ["Guide", "Contents", "1. Click \"A\"", "2. Click \"New assignment\"", "3. Select \"Course\"", "Guide"]
                .iter()
                .map(|s| s.to_string())
                .collect();
        assert!(bookmark_problems(&good, &wanted).is_empty());
        // The real titles WebView2 wrote with the Segoe UI font on 2026-11-01 (see `style` in the export crate).
        let doubled: Vec<String> = [
            "Guide",
            "ContentsContents",
            "1. Click \"A\"",
            "2.2. Click \"New assignment\" Click \"New assignment\"",
            "3.3. Select \"Course\" Select \"Course\"",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let problems = bookmark_problems(&doubled, &wanted);
        assert!(problems.iter().any(|p| p.contains("no bookmark \"2. Click")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("again") && p.contains("2.2.")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("again") && p.contains("3.3.")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("ContentsContents")), "{problems:?}");
        let missing: Vec<String> = good[..4].to_vec();
        assert_eq!(bookmark_problems(&missing, &wanted), ["no bookmark \"3. Select \\\"Course\\\"\""]);
    }

    #[test]
    fn headings_in_letters_roboto_lacks_still_get_one_bookmark() {
        let Some(_engine) = engine_present("headings_in_letters_roboto_lacks_still_get_one_bookmark") else {
            return;
        };
        // Roboto has no Japanese, so the engine falls back to a Windows font for these letters.
        let mut d = demo();
        d.document.guide.steps[0].title = "「保存」をクリック".to_string();
        let plan = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default());
        let options = PdfOptions { new_page_per_step: true, ..PdfOptions::default() };
        let export = export_pdf(&plan, &options, &WebView2Pdf).unwrap();
        let found = titles(&export.files[0].bytes);
        let wanted = [
            "How to submit an assignment",
            "Contents",
            "1. 「保存」をクリック",
            "2. Click \"New assignment\"",
            "2.1. Type \"Week 3 reflection\"",
            "3. Select \"Course\"",
            "4. Click \"Submit\"",
        ];
        let problems = bookmark_problems(&found, &wanted);
        assert!(problems.is_empty(), "{problems:?} in {found:?}");
    }

    #[test]
    fn the_title_reader_reads_plain_hex_and_escaped_strings() {
        assert_eq!(titles(b"<</Title (Plain one)>> <</Title(Tight)>>"), ["Plain one", "Tight"]);
        assert_eq!(titles(b"<</Title <FEFF00480069>>>"), ["Hi"]);
        assert_eq!(titles(b"<</Title (a \\(b\\) \\\\ \\101)>>"), ["a (b) \\ A"]);
        assert_eq!(titles(b"no titles here"), Vec::<String>::new());
    }

    #[test]
    fn every_layout_option_gives_one_correct_bookmark_for_each_heading_and_no_stray_pages() {
        let Some(_engine) =
            engine_present("every_layout_option_gives_one_correct_bookmark_for_each_heading_and_no_stray_pages")
        else {
            return;
        };
        let plan = demo_plan();
        // The top-level steps and notes: step 1, step 2 (with its substep), step 3, the tip, and step 4.
        let top_level = plan.steps.iter().filter(|s| s.depth == 0).count();
        assert_eq!(top_level, 5);
        let expected = [
            "How to submit an assignment",
            "1. Click \"Assignments\"",
            "2. Click \"New assignment\"",
            "2.1. Type \"Week 3 reflection\"",
            "3. Select \"Course\"",
            "4. Click \"Submit\"",
        ];
        for (label, options, exact_pages) in [
            ("default", PdfOptions::default(), None),
            ("with a cover", PdfOptions { cover_page: true, ..PdfOptions::default() }, None),
            (
                "a page for every step",
                PdfOptions { new_page_per_step: true, ..PdfOptions::default() },
                Some(1 + top_level),
            ),
            (
                "cover and a page for every step",
                PdfOptions { cover_page: true, new_page_per_step: true, ..PdfOptions::default() },
                Some(2 + top_level),
            ),
            (
                "a page for every step, no contents",
                PdfOptions { new_page_per_step: true, contents: false, ..PdfOptions::default() },
                Some(1 + top_level),
            ),
            (
                "Letter landscape, a page for every step",
                PdfOptions {
                    page_size: PageSize::Letter,
                    orientation: Orientation::Landscape,
                    new_page_per_step: true,
                    ..PdfOptions::default()
                },
                None,
            ),
        ] {
            let export = export_pdf(&plan, &options, &WebView2Pdf).unwrap_or_else(|e| panic!("{label}: {e}"));
            let pdf = &export.files[0].bytes;
            // Set EXPRESS_TEST_PDF_DIR to keep the files for looking at with other programs.
            if let Some(dir) = std::env::var_os("EXPRESS_TEST_PDF_DIR") {
                let _ = fs::create_dir_all(&dir);
                let _ = fs::write(Path::new(&dir).join(format!("{}.pdf", label.replace([' ', ','], "-"))), pdf);
            }
            let found = titles(pdf);
            let mut wanted = expected.to_vec();
            if options.contents {
                wanted.push("Contents");
            }
            let problems = bookmark_problems(&found, &wanted);
            assert!(problems.is_empty(), "{label}: {problems:?} in {found:?}");
            let audit = audit_pdf(pdf);
            if let Some(pages) = exact_pages {
                assert_eq!(
                    audit.pages, pages,
                    "{label}: one page for the front and one for each step, and no blank one"
                );
            }
            assert!(audit.tagged && audit.bookmarks, "{label}: {audit:?}");
        }
    }

    #[test]
    fn two_exports_in_a_row_both_work_and_leave_no_folders_behind() {
        let Some(_engine) = engine_present("two_exports_in_a_row_both_work_and_leave_no_folders_behind") else {
            return;
        };
        let before = leftover_folders();
        let plan = demo_plan();
        for _ in 0..2 {
            let export = export_pdf(&plan, &PdfOptions::default(), &WebView2Pdf).unwrap();
            assert!(audit_pdf(&export.files[0].bytes).is_pdf);
        }
        assert_eq!(leftover_folders(), before, "the engine's working folders were removed");
    }

    fn leftover_folders() -> usize {
        fs::read_dir(std::env::temp_dir())
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter(|e| e.file_name().to_string_lossy().starts_with("openstage-express-pdf-"))
                    .count()
            })
            .unwrap_or(0)
    }
}
