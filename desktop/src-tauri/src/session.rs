//! The backend's reading session, as one state machine — the counterpart to
//! the webview's `desktop/src/lib/session.ts`.
//!
//! Ten of the commands require a session and used to say so each for itself:
//! seven separate `ok_or("no chart has been built yet")` checks over an
//! `Option<ChartData>` in a seven-field struct that nothing owned. The invariant
//! lived in prose (CONTEXT.md states it) and in whichever check the next command
//! remembered to copy.
//!
//! Here it lives in the type. A session is **idle**, **open**, **recording**, or
//! **transcribing**, and every way to reach a reading goes through one guard, so
//! a command cannot forget it. Refusals are values with the reason attached,
//! matching the stance the webview's machine already takes.
//!
//! Two facts the old shape could not express, and this one does:
//!
//!   * **Never recording without a reading.** The pair `(Option<ChartData>,
//!     Option<Recorder>)` had four states for three real ones, and the fourth —
//!     a recorder with no chart to append to — was ruled out only by a check in
//!     `start_recording`.
//!   * **A stopped take is not a finished take.** The recorder leaves the
//!     session before its words are transcribed, which took real time and left
//!     the old state indistinguishable from idle-but-open. A second take could
//!     start in that window and be handed the same `session_secs` offset as the
//!     one still landing, so both takes' folio anchors would claim the same
//!     stretch of the recording. `Transcribing` closes that window.
//!
//! The capture device sits behind [`Capture`] so the session can be driven in a
//! test without a microphone: production passes `record::start`, the tests pass
//! a fake that yields a known duration.

use astro::contract::{ChartData, Segment};
use astro::route::{Filing, route_into};
use std::path::PathBuf;

/// The one refusal, in one place. Every command that needs a reading and has
/// none fails with exactly this.
pub const NO_READING: &str = "no chart has been built yet";

/// Something capturing audio into a file — the port the recorder sits behind.
///
/// Two adapters justify it: `record::Recorder` in the app, and a fake in this
/// module's tests, which is what makes the take arithmetic below testable at
/// all (it used to be reachable only by running the app with a real microphone
/// and a real ggml model).
pub trait Capture: Send {
    /// Stop capturing; return the WAV written and how many seconds it holds.
    fn stop(self: Box<Self>) -> Result<(PathBuf, f64), String>;
}

/// The reading a session is working on, and everything the session accumulates
/// onto it.
#[derive(Debug)]
pub struct Reading {
    /// The chart's excerpt list is authoritative — takes append to it and
    /// curation (merge/correct) edits it in place.
    pub chart: ChartData,
    /// `{readings_dir}/{name}_{date}/` when a readings folder is configured —
    /// chart.json and transcriptions auto-save here through the session.
    pub dir: Option<PathBuf>,
    /// Suggested export name, `{name}_{date}.html`.
    pub artifact_name: String,
    /// Highest `take-{n}.jsonl` ordinal so far — seeded from the folder on
    /// reopen so a new take never overwrites one already on disk.
    takes: usize,
    /// Total seconds recorded into this reading. Offsets each new take's
    /// timestamps so folio anchors run continuously across takes.
    secs: f64,
}

impl Reading {
    /// A freshly built reading: nothing recorded into it yet.
    pub fn new(chart: ChartData, dir: Option<PathBuf>, artifact_name: String) -> Reading {
        Reading { chart, dir, artifact_name, takes: 0, secs: 0.0 }
    }

    /// A reading reopened from the library, continuing past the takes already
    /// in its folder.
    pub fn reopened(
        chart: ChartData,
        dir: Option<PathBuf>,
        artifact_name: String,
        takes: usize,
    ) -> Reading {
        Reading { chart, dir, artifact_name, takes, secs: 0.0 }
    }
}

/// The open working state.
#[derive(Default)]
pub enum Session {
    #[default]
    Idle,
    Open(Reading),
    /// A capture is running into the reading.
    Recording { reading: Reading, capture: Box<dyn Capture>, model: PathBuf },
    /// The capture has stopped and its words are being transcribed. The take
    /// has not landed yet, so no other take may start or land.
    Transcribing { reading: Reading },
}

/// A take that has stopped recording but not yet landed: what the caller needs
/// to transcribe it, plus the arithmetic that must not be re-derived.
#[derive(Debug, PartialEq)]
pub struct PendingTake {
    pub wav: PathBuf,
    pub model: PathBuf,
    pub locale: astro::i18n::Locale,
    /// Seconds recorded into this reading *before* this take. Every segment's
    /// start shifts by it, so anchors run continuously.
    offset: f64,
    /// This take's own duration.
    secs: f64,
}

/// A take that landed: what to persist, and anything to surface.
#[derive(Debug, PartialEq)]
pub struct LandedTake {
    /// The take's segments as JSONL, already session-offset so the file matches
    /// the folio anchors.
    pub jsonl: String,
    /// The name to write it under, `take-{n}.jsonl`.
    pub filename: String,
    /// Verify-gate rejections from routing this take.
    pub warnings: Vec<String>,
}

impl Session {
    /// The reading, or the one refusal. This is the guard ten commands used to
    /// spell out for themselves.
    pub fn reading(&self) -> Result<&Reading, String> {
        match self {
            Session::Idle => Err(NO_READING.to_string()),
            Session::Open(r)
            | Session::Recording { reading: r, .. }
            | Session::Transcribing { reading: r } => Ok(r),
        }
    }

    /// The reading, mutably — curation edits the chart in place, and is
    /// deliberately allowed mid-take: merging a passage while the recorder runs
    /// must not be refused, and the take appends to whatever it finds.
    pub fn reading_mut(&mut self) -> Result<&mut Reading, String> {
        match self {
            Session::Idle => Err(NO_READING.to_string()),
            Session::Open(r)
            | Session::Recording { reading: r, .. }
            | Session::Transcribing { reading: r } => Ok(r),
        }
    }

    /// The chart, or the one refusal — for the readers (export, filename).
    pub fn chart(&self) -> Result<&ChartData, String> {
        self.reading().map(|r| &r.chart)
    }

    pub fn is_recording(&self) -> bool {
        matches!(self, Session::Recording { .. })
    }

    /// Why this session cannot take a new geometry for the same reading, or
    /// `None`. Recalculating mid-take would move the passages a take is
    /// currently appending to — the same stance the webview takes.
    fn why_cannot_settle(&self) -> Option<&'static str> {
        match self {
            Session::Recording { .. } => Some("stop recording first"),
            Session::Transcribing { .. } => Some("wait for the take to finish transcribing"),
            _ => None,
        }
    }

    /// Open a reading, replacing whatever was open. Refused mid-take: the take
    /// would otherwise land on a reading that is no longer the one it was
    /// recorded into — the defect the webview's machine was built to prevent,
    /// enforced here too so it holds even if a control is left ungated.
    pub fn open(&mut self, reading: Reading) -> Result<(), String> {
        if let Some(why) = self.why_cannot_settle() {
            return Err(why.to_string());
        }
        *self = Session::Open(reading);
        Ok(())
    }

    /// Replace the open reading's chart with a recomputed one (a reproject),
    /// keeping the session's dir, export name and take bookkeeping.
    pub fn resettle(&mut self, chart: ChartData) -> Result<&ChartData, String> {
        if let Some(why) = self.why_cannot_settle() {
            return Err(why.to_string());
        }
        let reading = self.reading_mut()?;
        reading.chart = chart;
        Ok(&reading.chart)
    }

    /// Begin a take. Guards first and only then calls `start`, so a refusal
    /// never opens the capture device.
    pub fn begin_take(
        &mut self,
        model: PathBuf,
        start: impl FnOnce() -> Result<Box<dyn Capture>, String>,
    ) -> Result<(), String> {
        match self {
            Session::Recording { .. } => return Err("already recording".to_string()),
            Session::Transcribing { .. } => {
                return Err("wait for the take to finish transcribing".to_string());
            }
            Session::Idle => return Err(NO_READING.to_string()),
            Session::Open(_) => {}
        }
        let capture = start()?;
        let Session::Open(reading) = std::mem::take(self) else {
            unreachable!("matched Open above");
        };
        *self = Session::Recording { reading, capture, model };
        Ok(())
    }

    /// Stop the capture. The session moves to `Transcribing` — the take is not
    /// finished, and nothing else may start until it lands or is abandoned.
    pub fn end_take(&mut self) -> Result<PendingTake, String> {
        if !self.is_recording() {
            // Not recording covers idle too; the more specific message wins.
            return Err(match self {
                Session::Idle => NO_READING.to_string(),
                _ => "not recording".to_string(),
            });
        }
        let Session::Recording { reading, capture, model } = std::mem::take(self) else {
            unreachable!("checked is_recording above");
        };
        let locale = astro::i18n::Locale::parse(&reading.chart.meta.locale);
        let offset = reading.secs;
        // Stop before re-seating the session, so a capture that fails to close
        // leaves the reading open rather than stuck mid-take.
        let stopped = capture.stop();
        *self = Session::Transcribing { reading };
        let (wav, secs) = stopped.inspect_err(|_| self.abandon_take())?;
        Ok(PendingTake { wav, model, locale, offset, secs })
    }

    /// Give up on a pending take — transcription failed. The reading stays
    /// open, so the astrologer can simply record again.
    pub fn abandon_take(&mut self) {
        if let Session::Transcribing { .. } = self {
            let Session::Transcribing { reading } = std::mem::take(self) else {
                unreachable!("matched Transcribing above");
            };
            *self = Session::Open(reading);
        }
    }

    /// Land a transcribed take: shift its segments onto the session clock,
    /// route them into the chart *after* the passages already there, and count
    /// the take.
    ///
    /// Routing appends rather than re-indexing, so earlier curation (merges,
    /// corrections) survives every take.
    pub fn land_take(
        &mut self,
        pending: PendingTake,
        mut segments: Vec<Segment>,
    ) -> Result<LandedTake, String> {
        if !matches!(self, Session::Transcribing { .. }) {
            return Err("no take is being transcribed".to_string());
        }
        for seg in &mut segments {
            seg.start += pending.offset;
        }
        let jsonl = astro::transcribe::to_jsonl(&segments);
        let take = astro::route::Transcript::from_segments(segments);

        let Session::Transcribing { mut reading } = std::mem::take(self) else {
            unreachable!("checked Transcribing above");
        };
        // Append, not replace, so earlier curation survives every take.
        let warnings = route_into(&mut reading.chart, &take, Filing::Append).warnings;
        reading.secs = pending.offset + pending.secs;
        reading.takes += 1;
        let filename = format!("take-{}.jsonl", reading.takes);
        *self = Session::Open(reading);
        Ok(LandedTake { jsonl, filename, warnings })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use astro::contract::Excerpt;

    /// The second adapter behind [`Capture`]: no device, a duration we choose.
    struct FakeCapture {
        secs: f64,
    }
    impl Capture for FakeCapture {
        fn stop(self: Box<Self>) -> Result<(PathBuf, f64), String> {
            Ok((PathBuf::from("/tmp/fake-take.wav"), self.secs))
        }
    }

    struct FailingCapture;
    impl Capture for FailingCapture {
        fn stop(self: Box<Self>) -> Result<(PathBuf, f64), String> {
            Err("the device went away".to_string())
        }
    }

    fn chart() -> ChartData {
        let input = astro::chart::BirthInput {
            name: "T".into(),
            date: "1990-07-13".parse().unwrap(),
            time: "14:30:00".parse().unwrap(),
            lat: 52.52,
            lon: 13.405,
            tz: chrono_tz::Europe::Berlin,
            place: "Berlin".into(),
            locale: astro::i18n::Locale::En,
            house_system: astro::chart::systems::house_system("whole-sign"),
            ayanamsa: None,
        };
        astro::chart::compute_chart(&input).unwrap()
    }

    fn open() -> Session {
        let mut s = Session::default();
        s.open(Reading::new(chart(), None, "t.html".into())).unwrap();
        s
    }

    fn capture(secs: f64) -> impl FnOnce() -> Result<Box<dyn Capture>, String> {
        move || Ok(Box::new(FakeCapture { secs }))
    }

    fn seg(start: f64, text: &str) -> Segment {
        Segment { start, text: text.to_string() }
    }

    #[test]
    fn an_idle_session_refuses_every_reader_with_one_message() {
        let s = Session::default();
        assert_eq!(s.reading().unwrap_err(), NO_READING);
        assert_eq!(s.chart().unwrap_err(), NO_READING);
        let mut s = s;
        assert_eq!(s.reading_mut().unwrap_err(), NO_READING);
        assert_eq!(s.resettle(chart()).unwrap_err(), NO_READING);
        assert_eq!(s.end_take().unwrap_err(), NO_READING);
        assert_eq!(s.begin_take(PathBuf::new(), capture(1.0)).unwrap_err(), NO_READING);
    }

    #[test]
    fn a_take_cannot_start_without_a_reading_and_the_device_is_never_opened() {
        let mut s = Session::default();
        let mut started = false;
        let err = s
            .begin_take(PathBuf::new(), || {
                started = true;
                Ok(Box::new(FakeCapture { secs: 1.0 }))
            })
            .unwrap_err();
        assert_eq!(err, NO_READING);
        assert!(!started, "a refused take must not open the capture device");
        assert!(!s.is_recording());
    }

    #[test]
    fn recording_is_never_entered_twice() {
        let mut s = open();
        s.begin_take("m.bin".into(), capture(5.0)).unwrap();
        assert!(s.is_recording());
        assert_eq!(s.begin_take("m.bin".into(), capture(5.0)).unwrap_err(), "already recording");
    }

    #[test]
    fn a_take_carries_the_model_and_locale_it_was_started_with() {
        let mut s = open();
        s.begin_take("/models/ggml.bin".into(), capture(12.5)).unwrap();
        let pending = s.end_take().unwrap();
        assert_eq!(pending.model, PathBuf::from("/models/ggml.bin"));
        assert_eq!(pending.locale, astro::i18n::Locale::En);
        assert_eq!(pending.secs, 12.5);
        assert_eq!(pending.offset, 0.0, "the first take starts at zero");
    }

    /// The invariant the old shape could not hold: a take that has stopped is
    /// still in flight until its words land, and nothing else may begin.
    #[test]
    fn a_second_take_cannot_start_while_the_first_is_still_transcribing() {
        let mut s = open();
        s.begin_take("m.bin".into(), capture(30.0)).unwrap();
        let pending = s.end_take().unwrap();

        assert!(!s.is_recording());
        assert_eq!(
            s.begin_take("m.bin".into(), capture(10.0)).unwrap_err(),
            "wait for the take to finish transcribing"
        );
        assert_eq!(s.end_take().unwrap_err(), "not recording");
        // The reading is still reachable throughout — curation keeps working.
        assert!(s.reading().is_ok());

        s.land_take(pending, vec![seg(0.0, "The sun.")]).unwrap();
        // Once it lands, the next take may begin.
        s.begin_take("m.bin".into(), capture(10.0)).unwrap();
    }

    /// Anchors must run continuously across takes: each take's segments shift
    /// by everything recorded before it, and the file written beside the chart
    /// carries the same shifted values as the folio.
    #[test]
    fn takes_accumulate_on_one_session_clock() {
        let mut s = open();

        s.begin_take("m.bin".into(), capture(60.0)).unwrap();
        let first = s.end_take().unwrap();
        let landed = s.land_take(first, vec![seg(0.0, "The sun rules this chart.")]).unwrap();
        assert_eq!(landed.filename, "take-1.jsonl");
        assert!(landed.jsonl.contains("\"start\":0.0"), "{}", landed.jsonl);

        s.begin_take("m.bin".into(), capture(45.0)).unwrap();
        let second = s.end_take().unwrap();
        assert_eq!(second.offset, 60.0, "the second take starts where the first ended");
        let landed = s.land_take(second, vec![seg(0.0, "The moon answers it.")]).unwrap();
        assert_eq!(landed.filename, "take-2.jsonl", "ordinals never collide");
        assert!(landed.jsonl.contains("\"start\":60.0"), "{}", landed.jsonl);

        s.begin_take("m.bin".into(), capture(1.0)).unwrap();
        assert_eq!(s.end_take().unwrap().offset, 105.0, "60 + 45");
    }

    /// A reopened reading continues past the takes already in its folder.
    #[test]
    fn a_reopened_reading_numbers_takes_past_the_ones_on_disk() {
        let mut s = Session::default();
        s.open(Reading::reopened(chart(), None, "t.html".into(), 3)).unwrap();
        s.begin_take("m.bin".into(), capture(1.0)).unwrap();
        let pending = s.end_take().unwrap();
        let landed = s.land_take(pending, vec![seg(0.0, "More words.")]).unwrap();
        assert_eq!(landed.filename, "take-4.jsonl");
    }

    #[test]
    fn landing_a_take_appends_to_the_passages_already_there() {
        let mut s = open();
        s.reading_mut().unwrap().chart.excerpts.push(Excerpt {
            id: "x1".into(),
            time: String::new(),
            span: [0, 0],
            text: "Curated by hand.".into(),
            tags: vec![],
        });

        s.begin_take("m.bin".into(), capture(10.0)).unwrap();
        let pending = s.end_take().unwrap();
        s.land_take(pending, vec![seg(0.0, "The sun is in cancer.")]).unwrap();

        let excerpts = &s.reading().unwrap().chart.excerpts;
        assert!(excerpts.len() > 1, "the take appended rather than replacing");
        assert_eq!(excerpts[0].id, "x1", "earlier curation survives the take");
        assert_eq!(excerpts[0].text, "Curated by hand.");
    }

    #[test]
    fn a_capture_that_fails_to_close_leaves_the_reading_open() {
        let mut s = open();
        s.begin_take("m.bin".into(), || Ok(Box::new(FailingCapture))).unwrap();
        assert_eq!(s.end_take().unwrap_err(), "the device went away");
        assert!(!s.is_recording());
        assert!(s.reading().is_ok(), "the reading must not be lost with the take");
        // And recording can simply begin again.
        s.begin_take("m.bin".into(), capture(5.0)).unwrap();
    }

    #[test]
    fn abandoning_a_take_returns_to_the_open_reading_and_keeps_the_clock() {
        let mut s = open();
        s.begin_take("m.bin".into(), capture(20.0)).unwrap();
        let first = s.end_take().unwrap();
        s.land_take(first, vec![seg(0.0, "One.")]).unwrap();

        s.begin_take("m.bin".into(), capture(99.0)).unwrap();
        let _abandoned = s.end_take().unwrap();
        s.abandon_take();

        assert!(s.reading().is_ok());
        // The abandoned take never landed, so it never advanced the clock.
        s.begin_take("m.bin".into(), capture(1.0)).unwrap();
        assert_eq!(s.end_take().unwrap().offset, 20.0);
    }

    #[test]
    fn a_take_cannot_land_when_none_is_in_flight() {
        let mut s = open();
        s.begin_take("m.bin".into(), capture(5.0)).unwrap();
        let pending = s.end_take().unwrap();
        s.land_take(pending, vec![seg(0.0, "Words.")]).unwrap();

        let stray = PendingTake {
            wav: "/tmp/x.wav".into(),
            model: "m.bin".into(),
            locale: astro::i18n::Locale::En,
            offset: 0.0,
            secs: 1.0,
        };
        assert_eq!(
            s.land_take(stray, vec![seg(0.0, "Words.")]).unwrap_err(),
            "no take is being transcribed"
        );
    }

    /// Leaving or reopening mid-take is refused rather than reconciled — the
    /// stance CONTEXT.md records, now enforced on this side too.
    #[test]
    fn a_reading_cannot_be_swapped_out_from_under_a_take() {
        let mut s = open();
        s.begin_take("m.bin".into(), capture(5.0)).unwrap();

        assert_eq!(
            s.open(Reading::new(chart(), None, "other.html".into())).unwrap_err(),
            "stop recording first"
        );
        assert_eq!(s.resettle(chart()).unwrap_err(), "stop recording first");

        let pending = s.end_take().unwrap();
        assert_eq!(
            s.open(Reading::new(chart(), None, "other.html".into())).unwrap_err(),
            "wait for the take to finish transcribing"
        );
        s.land_take(pending, vec![seg(0.0, "Words.")]).unwrap();
        // Open again once nothing is in flight.
        s.open(Reading::new(chart(), None, "other.html".into())).unwrap();
    }

    /// A reproject keeps everything the session accumulated — only the geometry
    /// changes.
    #[test]
    fn resettling_keeps_the_session_bookkeeping() {
        let mut s = Session::default();
        s.open(Reading::reopened(chart(), Some("/lib/mira".into()), "mira.html".into(), 2))
            .unwrap();
        s.begin_take("m.bin".into(), capture(10.0)).unwrap();
        let pending = s.end_take().unwrap();
        s.land_take(pending, vec![seg(0.0, "The sun.")]).unwrap();

        let mut recomputed = chart();
        recomputed.meta.system = "Placidus".into();
        recomputed.excerpts = s.reading().unwrap().chart.excerpts.clone();
        s.resettle(recomputed).unwrap();

        let r = s.reading().unwrap();
        assert_eq!(r.chart.meta.system, "Placidus");
        assert_eq!(r.dir.as_deref(), Some(std::path::Path::new("/lib/mira")));
        assert_eq!(r.artifact_name, "mira.html");
        assert!(!r.chart.excerpts.is_empty());
        // The clock and the take count are the session's, not the chart's.
        s.begin_take("m.bin".into(), capture(1.0)).unwrap();
        let pending = s.end_take().unwrap();
        assert_eq!(pending.offset, 10.0);
        assert_eq!(s.land_take(pending, vec![seg(0.0, "More.")]).unwrap().filename, "take-4.jsonl");
    }
}
