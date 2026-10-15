use std::cmp::Ordering;

use midly::Smf;
use yansi::{Color, Paint as _};

use crate::SAMPLE_RATE;
use crate::config::{Config, SynthConfig};
use crate::note::Note;
use crate::track::{EventKind, Track};

#[derive(Debug)]
struct TrackState<'a> {
	active: [Vec<Note>; 16],
	track: Track<'a>,
}

impl TrackState<'_> {
	fn next_sample(&mut self, conf: &SynthConfig) -> [f32; 2] {
		let mut track_sample = 0f32;

		for chan in &mut self.active {
			chan.retain_mut(|note| {
				let Some(sample) = note.next(conf.wave) else {
					return false;
				};
				track_sample += sample;
				true
			});
		}

		track_sample *= conf.volume;
		[track_sample * (1.0 - conf.pan), track_sample * conf.pan]
	}
}

pub struct MidiPlayer<'a> {
	conf: &'a Config,
	print: bool,

	/// Counts up to `conf.ticks_per_sample`.
	t_frac: u32,
	/// Incremented when `t_frac` wraps.
	t_ticks: u32,
	/// Invariant: `conf.track.len() == tracks.len()`.
	tracks: Vec<TrackState<'a>>,
}

pub struct EndOfFile;

fn print_starts(tracks: &[Vec<midly::TrackEvent<'_>>]) {
	const DEFAULT_SAMPLES_PER_TICK: u32 = 120;

	let mut firsts: Vec<_> = tracks
		.iter()
		.enumerate()
		.filter_map(|(i, track)| {
			let mut t = 0;
			let first = track.iter().find_map(|event| {
				t += event.delta.as_int();
				if matches!(
					event.kind,
					midly::TrackEventKind::Midi {
						message: midly::MidiMessage::NoteOn { .. },
						..
					}
				) {
					Some(t)
				} else {
					None
				}
			});
			Some((i, first?))
		})
		.collect();
	firsts.sort_by_key(|&(_i, first)| first);
	eprintln!("track starts:");

	let ticks_per_sec = SAMPLE_RATE / DEFAULT_SAMPLES_PER_TICK;
	for (i, first) in firsts {
		let t = first as f32 / ticks_per_sec as f32;
		eprintln!("track {i} @ {t:.2}");
	}
}

impl<'a> MidiPlayer<'a> {
	pub fn new(midi: Smf<'a>, conf: &'a Config, print: bool) -> Self {
		if midi.tracks.len() != conf.track.len() {
			eprintln!(
				"num tracks mismatch: {} in midi file, {} in config",
				midi.tracks.len(),
				conf.track.len()
			);
			print_starts(&midi.tracks);
			panic!();
		}

		let tracks = midi
			.tracks
			.into_iter()
			.map(|track| TrackState {
				track: Track::new(track),
				active: Default::default(),
			})
			.collect();

		Self {
			conf,
			print,

			t_frac: 0,
			t_ticks: 0,
			tracks,
		}
	}

	pub fn conf(&self) -> &'a Config {
		self.conf
	}

	fn on_tick(&mut self) -> Result<(), EndOfFile> {
		let mut playing = false;
		'track: for track in &mut self.tracks {
			loop {
				let Some(ev) = track.track.peek() else {
					continue 'track;
				};

				playing = true;

				match self.t_ticks.cmp(&ev.t) {
					Ordering::Less => continue 'track,
					Ordering::Equal => {}
					// Should never occur.
					Ordering::Greater => unreachable!("missed event"),
				}
				_ = track.track.next();

				let notes = &mut track.active[usize::from(ev.channel)];
				let existing = notes.iter_mut().find(|note| note.key() == ev.key);
				match (ev.kind, existing) {
					(EventKind::NoteOn { vel }, Some(existing)) => {
						existing.set_vel(vel);
					}
					(EventKind::NoteOn { vel }, None) => {
						notes.push(Note::new(ev.key, vel));
					}
					(EventKind::NoteOff, Some(existing)) => {
						existing.end();
					}
					(EventKind::NoteOff, None) => {}
				}

				// Loop to properly handle multiple simultaneous (delta == 0) events.
			}
		}

		if playing { Ok(()) } else { Err(EndOfFile) }
	}
}

pub type ActiveSet = [u8; 128];

impl MidiPlayer<'_> {
	pub fn active_set(&self) -> ActiveSet {
		let mut active_set = [0u8; 128];

		for (i, track) in self.tracks.iter().enumerate() {
			let i: u8 = i.try_into().unwrap();
			for note in track.active.iter().flatten() {
				active_set[usize::from(note.key())] = i;
			}
		}

		active_set
	}

	fn print_active(&mut self) {
		if !self.print {
			return;
		}

		for active_voice in self.active_set() {
			let colors = [
				Color::White,
				Color::BrightBlue,
				Color::BrightMagenta,
				Color::BrightYellow,
				Color::BrightGreen,
				Color::Red,
			];
			if active_voice > 0 {
				print!("{}", (active_voice).fg(colors[usize::from(active_voice)]));
			} else {
				print!(" ");
			}
		}

		println!();
	}

	pub fn jump_to_next_note(&mut self) -> Result<(), EndOfFile> {
		let next_midi_event_t = self
			.tracks
			.iter_mut()
			.filter_map(|track| track.track.peek())
			.map(|ev| ev.t)
			.min()
			.ok_or(EndOfFile)?;
		self.t_ticks = next_midi_event_t;
		self.t_frac = 0;

		Ok(())
	}
}

impl Iterator for MidiPlayer<'_> {
	type Item = [f32; 2];

	fn next(&mut self) -> Option<[f32; 2]> {
		if self.t_frac == 0 {
			self.on_tick().ok()?;
		}

		self.t_frac += 1;
		if self.t_frac == self.conf.samples_per_tick {
			self.t_frac = 0;
			self.t_ticks += 1;
		}

		if self.t_frac == 0 && self.t_ticks.is_multiple_of(20) {
			self.print_active();
		}

		let mut sample = [0f32; 2];
		for (track, conf) in self.tracks.iter_mut().zip(&self.conf.track) {
			let track_sample = track.next_sample(conf);
			sample[0] += track_sample[0];
			sample[1] += track_sample[1];
		}
		Some(sample)
	}
}
