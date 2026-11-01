// SPDX-License-Identifier: Apache-2.0
//! Playing a cue sound (spec section 4.2). The sounds themselves are made in `openstage_express_core::cues`; this plays one through
//! Windows' own `PlaySound`, from memory, without waiting for it to end. It plays through whatever the person has chosen as their
//! sound device, at the volume Windows gives the program.

use std::sync::OnceLock;

use openstage_express_core::cues::Cue;
use windows::core::PCWSTR;
use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

/// The cue as a WAV file that stays in memory for as long as the program runs (Windows reads the file while it plays, after
/// `PlaySoundW` has returned, so the bytes must not move or go away).
pub fn wav_of(cue: Cue) -> &'static [u8] {
    static WAVS: OnceLock<[Vec<u8>; 4]> = OnceLock::new();
    let all = WAVS.get_or_init(|| Cue::ALL.map(|c| c.wav()));
    let index = Cue::ALL.iter().position(|c| *c == cue).unwrap_or(0);
    &all[index]
}

/// Start playing `cue` and come back at once. A cue that is still playing is cut off by the next. Says whether Windows took it (it
/// does not when the computer has no sound device).
pub fn play(cue: Cue) -> bool {
    let wav = wav_of(cue);
    unsafe { PlaySoundW(PCWSTR(wav.as_ptr() as *const u16), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT).as_bool() }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Nothing here plays a sound: the tests run on someone's computer, and a cue is heard.

    #[test]
    fn each_cue_is_kept_in_memory_as_the_wav_file_the_core_makes() {
        for cue in Cue::ALL {
            let kept = wav_of(cue);
            assert_eq!(kept, cue.wav().as_slice(), "{cue:?}");
            assert_eq!(
                wav_of(cue).as_ptr(),
                kept.as_ptr(),
                "the same bytes every time, so they stay where Windows reads them"
            );
        }
        assert_ne!(wav_of(Cue::Start), wav_of(Cue::Step));
    }
}
