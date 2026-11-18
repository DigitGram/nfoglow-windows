// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 DigitGram
//! Background music: the built-in chiptunes by Stennish (CC BY 4.0, see assets/music/README.md), or the tracks of a
//! folder (OGG, MP3, FLAC, WAV, and the tracker modules MOD, XM, S3M, IT through xmrsplayer). Ported from music.py of
//! NfoGlow for Linux.
use rodio::Source;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What the player plays from a folder. Linux also knows .mptm and .opus (libopenmpt, GStreamer); rodio and
/// xmrsplayer do not.
pub const AUDIO: [&str; 10] = [".mod", ".xm", ".s3m", ".it", ".ogg", ".oga", ".mp3", ".flac", ".wav", ".m4a"];
const TRACKERS: [&str; 4] = [".mod", ".xm", ".s3m", ".it"];
const TRACKER_RATE: u32 = 48_000;
const TRACKER_PIECE: usize = 4096; // samples (both channels) rendered ahead at a time
// built-in tracks: intro 0-8 s, loop 8-48 s; the loop plays twice per track
const INTRO: f32 = 8.;
const LOOP_END: f32 = 48.;
const LOOPS: usize = 2;
// seconds: start of the music, end of each track, switching it off
pub const FADE_IN: f32 = 3.;
const FADE_END: f32 = 3.;
pub const FADE_OFF: f32 = 1.5;

/// A tracker module played by xmrsplayer, as a source for the player. The player borrows the module, so both live in a
/// thread of their own that renders a few pieces ahead; dropping the source ends the thread.
struct Tracker {
    pieces: std::sync::mpsc::Receiver<Vec<f32>>,
    piece: Vec<f32>,
    at: usize,
    length: f32, // seconds
}

impl Tracker {
    /// The module in bytes; suffix (".xm" ...) says which format, since a MOD has no sign that it is one.
    fn new(bytes: &[u8], suffix: &str) -> Result<Self, String> {
        use xmrs::prelude::Module;
        let module = match suffix {
            ".xm" => Module::load_xm(bytes),
            ".s3m" => Module::load_s3m(bytes),
            ".it" => Module::load_it(bytes),
            _ => Module::load_mod(bytes),
        }
        .map_err(|e| format!("{e:?}"))?;
        let length = xmrsplayer::prelude::XmrsPlayer::new(&module, TRACKER_RATE, 0).duration_seconds() as f32;
        if !(length > 0.5) {
            return Err("no music in it".into()); // nothing to play, or something that only looked like a module
        }
        let (send, pieces) = std::sync::mpsc::sync_channel(8);
        std::thread::spawn(move || {
            let mut player = xmrsplayer::prelude::XmrsPlayer::new(&module, TRACKER_RATE, 0);
            player.set_max_loop_count(1); // once through; a song that jumps back for ever ends at its loop
            // never longer than it says it is (and a few seconds of the last notes ringing out)
            let mut left = ((length + 5.) * TRACKER_RATE as f32 * 2.) as usize;
            while left > 0 {
                let piece: Vec<f32> = player.by_ref().take(TRACKER_PIECE.min(left)).map(|v| v as f32 / 32768.).collect();
                left = if piece.len() < TRACKER_PIECE { 0 } else { left - piece.len() };
                if piece.is_empty() || send.send(piece).is_err() {
                    break; // the end, or nobody listens any more
                }
            }
        });
        Ok(Tracker { pieces, piece: Vec::new(), at: 0, length })
    }
}

impl Iterator for Tracker {
    type Item = rodio::Sample;
    fn next(&mut self) -> Option<Self::Item> {
        if self.at >= self.piece.len() {
            self.piece = self.pieces.recv().ok()?;
            self.at = 0;
        }
        self.at += 1;
        self.piece.get(self.at - 1).copied()
    }
}

impl Source for Tracker {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> rodio::ChannelCount {
        rodio::ChannelCount::new(2).unwrap()
    }
    fn sample_rate(&self) -> rodio::SampleRate {
        rodio::SampleRate::new(TRACKER_RATE).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(self.length))
    }
}

/// The audio files of a folder, sorted by name.
pub fn tracks(folder: &Path) -> Vec<PathBuf> {
    let low = |p: &PathBuf| p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder).into_iter().flatten().flatten().map(|e| e.path()).collect();
    files.retain(|p| AUDIO.iter().any(|s| low(p).ends_with(s)));
    files.sort_by_key(low);
    files
}

/// A built-in track as it is played: its intro once and then its loop, twice, without a gap. samples: all channels
/// interleaved, per_second of them a second.
pub fn looped<T: Copy>(samples: &[T], per_second: usize) -> Vec<T> {
    let at = |seconds: f32| ((seconds * per_second as f32) as usize).min(samples.len());
    let mut out = samples[..at(LOOP_END)].to_vec();
    for _ in 1..LOOPS {
        out.extend_from_slice(&samples[at(INTRO)..at(LOOP_END)]);
    }
    out
}

/// How loud the player is set for a volume 0..1: fading in (0..1), towards the end of a track (seconds left, when
/// tracks end softly) and while switching off (1..0). Squared sounds more even.
pub fn gain(volume: f32, fade_in: f32, left: Option<f32>, off: Option<f32>) -> f32 {
    (volume * fade_in * left.map_or(1., |l| (l / FADE_END).clamp(0., 1.)) * off.unwrap_or(1.)).powi(2)
}

/// The loudness of a window of samples in dB (0 = full scale), never under -60.
pub fn loudness(window: &[f32]) -> f32 {
    let mean = window.iter().map(|s| s * s).sum::<f32>() / window.len().max(1) as f32;
    (10. * mean.log10()).max(-60.)
}

/// Turns the loudness of 25 ms windows (dB, in the order they are heard) into a pulse 0..1 that jumps on a beat and
/// fades out until the next one.
///
/// The loudness itself hardly moves in dense music: a chiptune stays within a few dB, so a glow that follows it only
/// flickers around the middle. A beat is a window that is clearly louder than the moment before it, so the pulse
/// follows that rise, measured against the strongest rises of the last second: loud and quiet tracks pulse alike,
/// small rises (hi-hats, the notes of a melody) stay under the threshold, a long tone flashes once and then rests.
pub struct Beat {
    avg: f32,
    reference: f32,
    pub value: f32,
}

impl Beat {
    pub const STEP: f32 = 0.025; // seconds per window
    const THRESHOLD: f32 = 0.5; // a rise must be at least half the recent strongest one to count
    const RELEASE: f32 = 0.18; // seconds for the pulse to fall to a third

    pub fn new() -> Self {
        Beat { avg: -60., reference: 3., value: 0. }
    }

    pub fn feed(&mut self, db: f32) -> f32 {
        let db = db.max(-60.);
        let heard = db.max(self.avg - 12.); // a moment of silence between two notes is no beat when the next one comes
        let rise = (heard - self.avg).max(0.);
        self.avg += (heard - self.avg) * (Self::STEP / 0.3); // the loudness of about the last 0.3 s
        self.reference = rise.max(3.).max(self.reference * (-Self::STEP).exp()); // the strongest rise of about the last second
        let loud = ((db + 30.) / 20.).clamp(0., 1.); // a soft beat pulses a little less
        let hit = ((rise / self.reference - Self::THRESHOLD) / ((1. - Self::THRESHOLD) * 0.85)).clamp(0., 1.) * (0.5 + 0.5 * loud);
        self.value = hit.max(self.value * (-Self::STEP / Self::RELEASE).exp());
        self.value
    }
}

/// A source that also tells how loud each 25 ms window of it is (in dB), for the beat.
struct Meter<S> {
    inner: S,
    window: Vec<f32>,
    levels: Arc<Mutex<VecDeque<f32>>>,
}

impl<S: Source> Iterator for Meter<S> {
    type Item = rodio::Sample;
    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.inner.next()?;
        self.window.push(sample);
        if self.window.len() as f32 >= self.inner.sample_rate().get() as f32 * self.inner.channels().get() as f32 * Beat::STEP {
            let mut levels = self.levels.lock().unwrap();
            levels.push_back(loudness(&self.window));
            let old = levels.len().saturating_sub(200); // nobody listens: keep the last five seconds
            levels.drain(..old);
            self.window.clear();
        }
        Some(sample)
    }
}

impl<S: Source> Source for Meter<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }
    fn channels(&self) -> rodio::ChannelCount {
        self.inner.channels()
    }
    fn sample_rate(&self) -> rodio::SampleRate {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}

const BUILTIN: [&[u8]; 9] = [
    include_bytes!("../assets/music/stennish-2020-06-10-906pm.ogg"),
    include_bytes!("../assets/music/stennish-2020-12-03-1146am.ogg"),
    include_bytes!("../assets/music/stennish-2020-12-04-247am.ogg"),
    include_bytes!("../assets/music/stennish-2020-12-05-1136am.ogg"),
    include_bytes!("../assets/music/stennish-2020-12-05-518am.ogg"),
    include_bytes!("../assets/music/stennish-2020-12-06-207pm.ogg"),
    include_bytes!("../assets/music/stennish-2020-12-06-256am.ogg"),
    include_bytes!("../assets/music/stennish-2020-12-08-105pm.ogg"),
    include_bytes!("../assets/music/stennish-2021-06-18-135am.ogg"),
];

/// One player for the whole app. tick() keeps it going: the next track, the fades. level(): the beat of what is
/// heard, 0..1, for the pulsing glow.
pub struct Player {
    _device: Option<rodio::MixerDeviceSink>, // the sound stops when it is dropped
    out: rodio::Player,
    queue: Vec<PathBuf>, // the tracks of the folder; empty: the built-in ones
    order: Vec<usize>,
    current: Option<usize>,
    length: Option<f32>, // seconds of the track that plays
    pub playing: bool,
    shuffle: bool,
    fade: bool,
    fade_in: f32,
    off: Option<f32>,
    volume: f32,
    errors: usize,
    seed: u64,
    levels: Arc<Mutex<VecDeque<f32>>>,
    beat: Beat,
}

impl Player {
    /// The player on the default sound device; an error when the computer has none.
    pub fn new() -> Result<Self, String> {
        let mut device = rodio::DeviceSinkBuilder::open_default_sink().map_err(|e| e.to_string())?;
        device.log_on_drop(false);
        let out = rodio::Player::connect_new(device.mixer());
        Ok(Self::with(out, Some(device)))
    }

    fn with(out: rodio::Player, device: Option<rodio::MixerDeviceSink>) -> Self {
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
        Player { _device: device, out, queue: Vec::new(), order: Vec::new(), current: None, length: None, playing: false, shuffle: true,
            fade: true, fade_in: 1., off: None, volume: 0.6, errors: 0, seed, levels: Arc::default(), beat: Beat::new() }
    }

    fn count(&self) -> usize {
        if self.queue.is_empty() { BUILTIN.len() } else { self.queue.len() }
    }

    /// Play the tracks of a folder, or the built-in ones when it has none. fade: start quietly (nothing blares
    /// out of the speakers) and let every track end softly.
    pub fn play(&mut self, folder: &str, shuffle: bool, volume: f32, fade: bool) {
        self.stop(false);
        self.queue = if folder.is_empty() { Vec::new() } else { tracks(Path::new(folder)) };
        (self.playing, self.shuffle, self.volume, self.fade, self.errors) = (true, shuffle, volume, fade, 0);
        self.fade_in = if fade { 0. } else { 1. };
        self.order.clear();
    }

    /// The next track into the player. Returns its (number, of how many, name: None for a built-in one) when it is
    /// another one than before.
    fn next(&mut self) -> Option<(usize, usize, Option<String>)> {
        if self.order.is_empty() {
            self.order = (0..self.count()).collect();
            if self.shuffle {
                for i in (1..self.order.len()).rev() {
                    self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    self.order.swap(i, (self.seed >> 33) as usize % (i + 1));
                }
            }
            self.order.reverse(); // taken from the end
        }
        let track = self.order.pop()?;
        let levels = self.levels.clone();
        let loaded = if self.queue.is_empty() {
            // the intro once and then the loop, as one piece: no gap
            rodio::Decoder::try_from(std::io::Cursor::new(BUILTIN[track])).map_err(|e| e.to_string()).map(|decoder| {
                let (channels, rate) = (decoder.channels(), decoder.sample_rate());
                let samples: Vec<f32> = decoder.collect();
                let played = looped(&samples, rate.get() as usize * channels.get() as usize);
                self.length = Some(played.len() as f32 / (rate.get() as f32 * channels.get() as f32));
                self.out.append(Meter { inner: rodio::buffer::SamplesBuffer::new(channels, rate, played), window: Vec::new(), levels });
            })
        } else if let Some(suffix) = TRACKERS.iter().find(|s| self.queue[track].to_string_lossy().to_lowercase().ends_with(*s)) {
            std::fs::read(&self.queue[track]).map_err(|e| e.to_string()).and_then(|bytes| Tracker::new(&bytes, suffix)).map(|tracker| {
                self.length = Some(tracker.length);
                self.out.append(Meter { inner: tracker, window: Vec::new(), levels });
            })
        } else {
            std::fs::File::open(&self.queue[track]).map_err(|e| rodio::decoder::DecoderError::IoError(e.to_string())).and_then(rodio::Decoder::try_from).map(|decoder| {
                self.length = decoder.total_duration().map(|d| d.as_secs_f32());
                self.out.append(Meter { inner: decoder, window: Vec::new(), levels });
            }).map_err(|e| e.to_string())
        };
        if loaded.is_err() {
            // e.g. a file that is no music: skip it, but give up when nothing plays
            self.errors += 1;
            if self.errors >= self.count().max(3) {
                self.stop(false);
            }
            return None;
        }
        self.errors = 0; // only failures in a row make the player give up
        let name = self.queue.get(track).map(|p| p.file_stem().unwrap_or_default().to_string_lossy().replace('_', " "));
        (self.current.replace(track) != Some(track)).then_some((track + 1, self.count(), name)) // one track on repeat is announced once
    }

    /// Call it often (dt: the seconds since the last call): the fades, and the next track when one has ended.
    /// Returns a track that starts now, to announce it (see next).
    pub fn tick(&mut self, dt: f32) -> Option<(usize, usize, Option<String>)> {
        if !self.playing {
            return None;
        }
        self.fade_in = (self.fade_in + dt / FADE_IN).min(1.);
        if let Some(off) = self.off.as_mut() {
            *off -= dt / FADE_OFF;
            if *off <= 0. {
                self.stop(false);
                return None;
            }
        }
        let started = if self.out.empty() { self.next() } else { None };
        let left = self.length.filter(|_| self.fade).map(|length| length - self.out.get_pos().as_secs_f32());
        self.out.set_volume(gain(self.volume, self.fade_in, left, self.off));
        started
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
    }

    /// The beat of what is heard now, or None when no music plays (the pulse then breathes on its own).
    pub fn level(&mut self) -> Option<f32> {
        for db in self.levels.lock().unwrap().drain(..) {
            self.beat.feed(db);
        }
        self.playing.then_some(self.beat.value)
    }

    /// Stop at once, or with fade over a second and a half.
    pub fn stop(&mut self, fade: bool) {
        if fade && self.playing {
            self.off.get_or_insert(1.);
            return;
        }
        (self.playing, self.current, self.off, self.length) = (false, None, None, None);
        self.out.skip_one(); // never more than one track is in it. Not clear(): that waits for the sound card
        self.levels.lock().unwrap().clear();
        self.beat = Beat::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_music_loops_without_a_gap() {
        let track: Vec<u32> = (0..60 * 4).collect(); // 60 s, 4 samples a second
        let played = looped(&track, 4);
        assert_eq!(played.len(), (48 + 40) * 4); // intro and loop, then the loop again
        assert_eq!((played[48 * 4 - 1], played[48 * 4], *played.last().unwrap()), (48 * 4 - 1, 8 * 4, 48 * 4 - 1));
        assert_eq!(looped(&track[..20], 4), [&track[..20], &[][..]].concat()); // a track shorter than its intro: once
    }

    #[test]
    fn music_fades() {
        assert_eq!((gain(1., 1., None, None), gain(0.5, 1., None, None), gain(1., 0., None, None)), (1., 0.25, 0.));
        assert!(gain(1., 0.5, None, None) < gain(1., 0.9, None, None)); // it starts quietly
        assert_eq!((gain(1., 1., Some(10.), None), gain(1., 1., Some(1.5), None), gain(1., 1., Some(0.), None)), (1., 0.25, 0.)); // every track ends softly
        assert_eq!(gain(1., 1., None, Some(0.5)), 0.25);
    }

    #[test]
    fn the_pulse_jumps_on_beats_not_on_loudness() {
        assert_eq!((loudness(&[1., -1., 1.]), loudness(&[0.; 8]), loudness(&[])), (0., -60., -60.));
        assert!((loudness(&[0.1; 100]) + 20.).abs() < 0.01);
        // a steady loud tone: one flash at its start, then rest
        let mut beat = Beat::new();
        let tone: Vec<f32> = (0..80).map(|_| beat.feed(-6.)).collect();
        assert!(tone[0] > 0.9 && tone[79] < 0.05, "{tone:?}");
        // a kick every half second over a quiet bed: a pulse on each kick, fading in between
        let mut beat = Beat::new();
        let run: Vec<f32> = (0..200).map(|i| beat.feed(if i % 20 == 0 { -4. } else { -22. })).collect();
        for kick in (40..200).step_by(20) {
            assert!(run[kick] > 0.8 && run[kick + 10] < run[kick] / 2. && run[kick + 19] < 0.1, "{kick}: {:?}", &run[kick..kick + 20]);
        }
        // small rises (the notes of a melody) stay under the threshold: the pulse only fades between the kicks
        let mut beat = Beat::new();
        let melody: Vec<f32> = (0..200).map(|i| beat.feed(if i % 20 == 0 { -4. } else if i % 5 == 0 { -19. } else { -22. })).collect();
        // the same numbers as music.Beat of NfoGlow for Linux gives for this run
        for (i, python) in [(40, 0.9255), (45, 0.4622), (50, 0.2308), (59, 0.0661), (100, 1.0), (105, 0.4994), (199, 0.0714)] {
            assert!((melody[i] - python).abs() < 0.001, "{i}: {} is not {python}", melody[i]);
        }
        // loud and quiet tracks pulse alike
        let mut quiet = Beat::new();
        let soft: Vec<f32> = (0..200).map(|i| quiet.feed(if i % 20 == 0 { -24. } else { -42. })).collect();
        assert!(soft[100] > 0.5);
    }

    /// A tiny ProTracker module: one square-wave sample, a C on the first row of one pattern (64 rows, 7.68 s).
    fn tiny_mod() -> Vec<u8> {
        let mut m = vec![0u8; 20]; // title
        for i in 0..31 {
            let mut sample = vec![0u8; 30];
            if i == 0 {
                sample[22..24].copy_from_slice(&32u16.to_be_bytes()); // 32 words long
                sample[25] = 64; // volume
                sample[28..30].copy_from_slice(&32u16.to_be_bytes()); // looped as a whole
            }
            m.extend(sample);
        }
        m.extend([1, 127]); // one position in the song
        m.extend([0u8; 128]);
        m.extend(b"M.K.");
        let mut pattern = vec![0u8; 64 * 4 * 4];
        pattern[..4].copy_from_slice(&[0x01, 0xac, 0x10, 0x00]); // sample 1, period 428
        m.extend(pattern);
        m.extend((0..64).map(|i| if i < 32 { 100u8 } else { 156 })); // the square wave
        m
    }

    #[test]
    fn tracker_modules_play_in_full() {
        let mut tracker = Tracker::new(&tiny_mod(), ".mod").unwrap();
        assert!((tracker.length - 7.68).abs() < 0.1, "{}", tracker.length);
        assert_eq!((tracker.channels().get(), tracker.sample_rate().get()), (2, 48_000));
        let samples: Vec<f32> = tracker.by_ref().collect(); // it ends: once through, then a short ring-out at most
        let seconds = samples.len() as f32 / 96_000.;
        assert!((7.5..13.).contains(&seconds), "{seconds}");
        assert!(samples.iter().any(|v| v.abs() > 0.05) && samples.iter().all(|v| v.abs() <= 1.)); // a note, not too loud
        // an XM file (from the xmrs crate's examples), and things that are no module
        if let Some(xm) = std::env::var_os("NFOGLOW_TEST_XM").and_then(|p| std::fs::read(p).ok()) {
            assert!(Tracker::new(&xm, ".xm").is_ok());
        }
        assert!(Tracker::new(b"no music at all", ".xm").is_err());
        assert!(Tracker::new(b"no music at all", ".mod").is_err());
        assert!(Tracker::new(&tiny_mod(), ".it").is_err()); // the suffix decides the format
    }

    #[test]
    fn tracks_of_a_folder() {
        let dir = std::env::temp_dir().join(format!("nfoglow-music-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["b.OGG", "a.mp3", "notes.txt", "c.flac", "d.XM", "e.mod"] {
            std::fs::write(dir.join(name), b"").unwrap();
        }
        assert_eq!(tracks(&dir), [dir.join("a.mp3"), dir.join("b.OGG"), dir.join("c.flac"), dir.join("d.XM"), dir.join("e.mod")]);
        assert!(tracks(&dir.join("missing")).is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// The player without a sound card: what it would send to the speakers is read here instead.
    #[test]
    fn the_player_plays_a_track_fades_it_in_and_goes_on_to_the_next() {
        let (out, mut heard) = rodio::Player::new();
        let mut player = Player::with(out, None);
        assert_eq!((player.level(), player.tick(1.)), (None, None)); // off: nothing
        player.play("", false, 1., true);
        assert_eq!(player.tick(0.), Some((1, 9, None))); // the first built-in track, announced
        assert_eq!(player.length, Some(88.)); // 48 s and the loop of 40 s again
        let (channels, rate) = (heard.channels().get() as usize, heard.sample_rate().get() as usize);
        let mut second = |player: &mut Player| {
            let samples: Vec<f32> = heard.by_ref().take(rate * channels).collect();
            player.tick(1.);
            loudness(&samples)
        };
        let quiet = second(&mut player);
        let (_, _, loud) = (second(&mut player), second(&mut player), second(&mut player));
        assert!(quiet < -40. && loud > quiet + 15. && loud > -30., "{quiet} {loud}"); // it starts quietly: faded in over 3 s
        assert!(player.level().is_some_and(|l| (0. ..=1.).contains(&l)));
        // a folder without music: the player gives up instead of trying for ever
        let dir = std::env::temp_dir().join(format!("nfoglow-player-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("broken.ogg"), b"no music").unwrap();
        player.play(dir.to_str().unwrap(), true, 1., false);
        for _ in 0..5 {
            second(&mut player); // the track before is dropped as the sound card asks for more
        }
        assert!(!player.playing);
        std::fs::remove_dir_all(dir).unwrap();
        // switching off fades out over a second and a half
        player.play("", true, 1., false);
        assert!(player.tick(0.).is_some());
        player.stop(true);
        player.tick(1.);
        assert!(player.playing);
        player.tick(1.);
        assert!(!player.playing && player.level().is_none());
    }
}
