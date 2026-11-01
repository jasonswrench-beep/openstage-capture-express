// SPDX-License-Identifier: Apache-2.0
//! The optional cue sounds (spec sections 4.2 and 8.9, P3): a quiet sound when capturing starts or stops, when a step is made and when
//! something goes wrong. They are off unless the person turns them on.
//!
//! The sounds are made here, from plain tones, so there are no sound files to ship or to take a licence for, and they are the same on
//! every computer. Each is a short WAV file held in memory; playing it is the job of the Windows crate. They are quiet on purpose
//! (the loudest point is about a quarter of full scale), start and end at silence so that they do not click, and are short enough
//! that a run of quick clicks does not turn into a drone.

/// What a cue tells the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cue {
    /// Capturing has started or been resumed: two notes going up.
    Start,
    /// Capturing has been paused or finished: the same two notes going down.
    Stop,
    /// A step was captured: one short soft note.
    Step,
    /// Something went wrong, or needs a look: a low double note.
    Error,
}

/// The samples per second of every cue.
pub const RATE: u32 = 22_050;

impl Cue {
    pub const ALL: [Cue; 4] = [Cue::Start, Cue::Stop, Cue::Step, Cue::Error];

    /// What the cue is called in words, for the button that plays it.
    pub fn name(self) -> &'static str {
        match self {
            Cue::Start => "start",
            Cue::Stop => "stop",
            Cue::Step => "step",
            Cue::Error => "problem",
        }
    }

    /// The notes of the cue: pitch in hertz, length in milliseconds, loudness from 0 to 1 of full scale.
    fn notes(self) -> (&'static [(f32, u32, f32)], u32) {
        match self {
            Cue::Start => (&[(523.25, 90, 0.22), (783.99, 90, 0.22)], 20),
            Cue::Stop => (&[(783.99, 90, 0.22), (523.25, 90, 0.22)], 20),
            Cue::Step => (&[(1046.5, 50, 0.12)], 0),
            Cue::Error => (&[(196.0, 110, 0.25), (196.0, 110, 0.25)], 50),
        }
    }

    /// The cue as a complete WAV file (16-bit, mono).
    pub fn wav(self) -> Vec<u8> {
        let (notes, gap_ms) = self.notes();
        let mut samples: Vec<i16> = Vec::new();
        for (n, (hz, ms, volume)) in notes.iter().enumerate() {
            if n > 0 {
                samples.extend(std::iter::repeat_n(0i16, (RATE * gap_ms / 1000) as usize));
            }
            samples.extend(note(*hz, *ms, *volume));
        }
        wav_file(&samples)
    }
}

/// One note: a sine tone whose first and last milliseconds fade in and out (a raised cosine), so it starts and ends at silence.
fn note(hz: f32, ms: u32, volume: f32) -> Vec<i16> {
    let count = (RATE * ms / 1000) as usize;
    let fade = ((RATE as usize * 10) / 1000).min(count / 2).max(1);
    (0..count)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let into = i.min(count - 1 - i);
            let envelope = if into >= fade {
                1.0
            } else {
                let x = into as f32 / fade as f32;
                (x * std::f32::consts::FRAC_PI_2).sin().powi(2)
            };
            let sample = (2.0 * std::f32::consts::PI * hz * t).sin() * envelope * volume;
            (sample * i16::MAX as f32).round() as i16
        })
        .collect()
}

fn wav_file(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // size of the format part
    out.extend_from_slice(&1u16.to_le_bytes()); // plain PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // one channel
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes()); // bytes per second
    out.extend_from_slice(&2u16.to_le_bytes()); // bytes per sample frame
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(wav: &[u8]) -> Vec<i16> {
        wav[44..].as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)).collect()
    }

    fn u32_at(wav: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([wav[at], wav[at + 1], wav[at + 2], wav[at + 3]])
    }

    #[test]
    fn every_cue_is_a_well_formed_wav_file() {
        for cue in Cue::ALL {
            let wav = cue.wav();
            assert_eq!(&wav[0..4], b"RIFF", "{cue:?}");
            assert_eq!(&wav[8..16], b"WAVEfmt ");
            assert_eq!(&wav[36..40], b"data");
            assert_eq!(u32_at(&wav, 4) as usize, wav.len() - 8, "the RIFF length covers the rest of the file");
            assert_eq!(u32_at(&wav, 40) as usize, wav.len() - 44, "the data length covers the samples");
            assert_eq!(u32_at(&wav, 24), RATE);
            assert_eq!(u32_at(&wav, 28), RATE * 2, "bytes per second");
            assert_eq!(u16::from_le_bytes([wav[20], wav[21]]), 1, "plain PCM");
            assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 1, "mono");
            assert_eq!(u16::from_le_bytes([wav[34], wav[35]]), 16, "16 bits");
            assert_eq!((wav.len() - 44) % 2, 0);
        }
    }

    #[test]
    fn the_cues_are_short_quiet_and_start_and_end_in_silence() {
        for cue in Cue::ALL {
            let s = samples(&cue.wav());
            let ms = s.len() as u32 * 1000 / RATE;
            assert!((40..=300).contains(&ms), "{cue:?} is {ms} ms");
            let peak = s.iter().map(|v| v.unsigned_abs()).max().unwrap() as f32 / i16::MAX as f32;
            assert!(peak > 0.05, "{cue:?} can be heard (peak {peak})");
            assert!(peak <= 0.26, "{cue:?} is quiet (peak {peak})");
            assert_eq!((s[0], *s.last().unwrap()), (0, 0), "{cue:?} does not click at either end");
        }
    }

    #[test]
    fn the_four_cues_sound_different_and_the_same_cue_always_does() {
        let all: Vec<Vec<u8>> = Cue::ALL.iter().map(|c| c.wav()).collect();
        for (i, a) in all.iter().enumerate() {
            for b in all.iter().skip(i + 1) {
                assert_ne!(a, b);
            }
        }
        assert_eq!(Cue::Step.wav(), Cue::Step.wav());
        // Stop is Start going the other way: the same length, with the high note first.
        assert_eq!(samples(&Cue::Start.wav()).len(), samples(&Cue::Stop.wav()).len());
    }

    /// How many times a run of samples crosses zero, which is twice the pitch in hertz times the seconds.
    fn crossings(s: &[i16]) -> usize {
        s.windows(2).filter(|w| (w[0] < 0) != (w[1] < 0) && w[0] != 0 && w[1] != 0).count()
    }

    #[test]
    fn the_notes_have_the_pitch_they_are_meant_to() {
        // The step cue is one note of 1046.5 Hz for 50 ms: about 105 crossings.
        let step = samples(&Cue::Step.wav());
        let expected = 2.0 * 1046.5 * 0.05;
        let got = crossings(&step) as f32;
        assert!((got - expected).abs() <= expected * 0.1, "step has {got} crossings, expected about {expected}");
        // The start cue rises: its second half crosses zero more often than its first.
        let start = samples(&Cue::Start.wav());
        let (first, second) = start.split_at(start.len() / 2);
        assert!(crossings(second) > crossings(first), "start goes up");
        let stop = samples(&Cue::Stop.wav());
        let (first, second) = stop.split_at(stop.len() / 2);
        assert!(crossings(second) < crossings(first), "stop goes down");
        // The problem cue is low: well under half the pitch of the step cue per millisecond.
        let error = samples(&Cue::Error.wav());
        let per_ms = |s: &[i16]| crossings(s) as f32 / (s.len() as f32 * 1000.0 / RATE as f32);
        assert!(per_ms(&error) < per_ms(&step) / 2.0);
    }

    #[test]
    fn each_cue_has_a_name_for_its_button() {
        let names: Vec<_> = Cue::ALL.iter().map(|c| c.name()).collect();
        assert_eq!(names, ["start", "stop", "step", "problem"]);
    }
}
