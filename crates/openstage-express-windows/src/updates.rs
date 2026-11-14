// SPDX-License-Identifier: Apache-2.0
//! Check for updates, the Windows part: the one network request the program makes, and opening a release page in the person's own
//! browser. The words and the rules are in `openstage_express_core::updates`; docs/update-check.md says what and why.
//!
//! The request is made with Windows' own HTTPS code (WinHTTP), so no web library is added to the program: HTTPS only, the certificate
//! checked by Windows, no redirect followed, no cookies, at most 2 MB read, and 15 seconds for each step. It carries a plain name for the
//! program and asks for the public list; nothing about the person, their guides or their computer is sent.

use std::path::Path;

use openstage_express_core::updates::{
    answer, is_release_page, release, turned_off, UpdateAnswer, RELEASES_API_HOST, RELEASES_API_PATH,
};

const USER_AGENT: &str = "OpenStageCaptureExpress-update-check";
/// The most of an answer that is read.
const MOST: usize = 2 * 1024 * 1024;
/// How long each part of the request may take: finding the server, connecting, sending, and waiting for the answer.
const TIMEOUT_MS: i32 = 15_000;

/// Check now, as Help > Check for updates asks. `program_dir` is the folder of the program, where `update-check.txt` may say `off`.
pub fn check(program_dir: Option<&Path>) -> UpdateAnswer {
    if turned_off(program_dir) {
        return UpdateAnswer::off();
    }
    match get(RELEASES_API_HOST, RELEASES_API_PATH) {
        Ok(text) => answer(release(), &text),
        Err(message) => UpdateAnswer::error(message),
    }
}

/// Open a page of Express's releases in the person's own browser, as opening a link from the Start menu does. Any other address is not
/// opened.
pub fn open_release_page(url: &str) -> bool {
    use windows::core::{w, HSTRING, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    if !is_release_page(url) {
        return false;
    }
    let result = unsafe { ShellExecuteW(None, w!("open"), &HSTRING::from(url), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
    // ShellExecuteW says it worked with a number above 32.
    result.0 as isize > 32
}

use std::ffi::c_void;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse,
    WinHttpSendRequest, WinHttpSetOption, WinHttpSetTimeouts, INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
    WINHTTP_DISABLE_COOKIES, WINHTTP_DISABLE_REDIRECTS, WINHTTP_FLAG_SECURE, WINHTTP_OPTION_DISABLE_FEATURE,
    WINHTTP_OPTION_REDIRECT_POLICY, WINHTTP_OPTION_REDIRECT_POLICY_NEVER, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
};

/// A WinHTTP handle that is closed when it goes.
struct Handle(*mut c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
}

const UNREACHABLE: &str = "GitHub could not be reached. Check the internet connection, or look at the releases page later.";

fn opened(raw: *mut c_void) -> Result<Handle, String> {
    if raw.is_null() {
        Err(UNREACHABLE.to_string())
    } else {
        Ok(Handle(raw))
    }
}

fn set_number(handle: &Handle, option: u32, value: u32) -> Result<(), String> {
    let bytes = value.to_ne_bytes();
    unsafe { WinHttpSetOption(Some(handle.0 as *const c_void), option, Some(&bytes)) }.map_err(|_| UNREACHABLE.to_string())
}

/// GET https://host/path, and the answer as text.
fn get(host: &str, path: &str) -> Result<String, String> {
    let agent = HSTRING::from(USER_AGENT);
    let host = HSTRING::from(host);
    let path = HSTRING::from(path);
    let verb = HSTRING::from("GET");
    unsafe {
        let session = opened(WinHttpOpen(&agent, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0))?;
        WinHttpSetTimeouts(session.0, TIMEOUT_MS, TIMEOUT_MS, TIMEOUT_MS, TIMEOUT_MS).map_err(|_| UNREACHABLE.to_string())?;
        set_number(&session, WINHTTP_OPTION_REDIRECT_POLICY, WINHTTP_OPTION_REDIRECT_POLICY_NEVER)?;
        let connection = opened(WinHttpConnect(session.0, &host, INTERNET_DEFAULT_HTTPS_PORT, 0))?;
        let request = opened(WinHttpOpenRequest(
            connection.0,
            &verb,
            &path,
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        ))?;
        set_number(&request, WINHTTP_OPTION_DISABLE_FEATURE, WINHTTP_DISABLE_REDIRECTS | WINHTTP_DISABLE_COOKIES)?;
        let headers: Vec<u16> = "Accept: application/vnd.github+json\r\n".encode_utf16().collect();
        WinHttpSendRequest(request.0, Some(&headers), None, 0, 0, 0).map_err(|_| UNREACHABLE.to_string())?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut()).map_err(|_| UNREACHABLE.to_string())?;

        let mut status = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(&mut status as *mut u32 as *mut c_void),
            &mut size,
            std::ptr::null_mut(),
        )
        .map_err(|_| UNREACHABLE.to_string())?;
        if status != 200 {
            return Err(format!("GitHub answered with an error ({status}). Try again later, or look at the releases page."));
        }

        let mut body: Vec<u8> = Vec::new();
        let mut chunk = vec![0u8; 64 * 1024];
        loop {
            let mut read = 0u32;
            WinHttpReadData(request.0, chunk.as_mut_ptr() as *mut c_void, chunk.len() as u32, &mut read)
                .map_err(|_| "The list of releases did not arrive whole. Try again.".to_string())?;
            if read == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..read as usize]);
            if body.len() > MOST {
                return Err("The list of releases was larger than expected, so it was not read.".into());
            }
        }
        String::from_utf8(body).map_err(|_| "The list of releases could not be read.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstage_express_core::updates::UpdateState;

    #[test]
    fn only_a_release_page_is_ever_opened() {
        for bad in ["https://example.org", "file:///C:/Windows/System32/calc.exe", "", "javascript:alert(1)"] {
            assert!(!open_release_page(bad), "{bad}");
        }
    }

    #[test]
    fn turned_off_it_asks_nothing_of_the_network() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("update-check.txt"), "off").unwrap();
        assert_eq!(check(Some(dir.path())).state, UpdateState::Off);
    }

    /// Reaches GitHub, so it is run only by hand: `cargo test -p openstage-express-windows -- --ignored`. It sends nothing but the
    /// request for the public list.
    #[test]
    #[ignore = "reaches GitHub over the network"]
    fn the_real_list_is_read_and_a_redirect_is_not_followed() {
        let text = get(RELEASES_API_HOST, RELEASES_API_PATH).expect("the list of releases");
        assert_eq!(answer("0.1.0-alpha.1", &text).state, UpdateState::Newer, "alpha 1 is older than what is published");
        // This address answers with a redirect to the newest release's page: it must come back as an error, not be followed.
        let said = get("github.com", "/jasonswrench-beep/openstage-capture-express/releases/latest").unwrap_err();
        assert!(said.contains("(302)"), "{said}");
    }
}
