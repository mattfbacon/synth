use std::num::NonZeroU8;

use midly::{MidiMessage, TrackEvent, TrackEventKind};

#[derive(Debug)]
struct TrackInner<'a> {
	data: Vec<TrackEvent<'a>>,
	i: usize,
	last_time: u32,
}

struct InnerEvent<'a> {
	pub t: u32,
	pub kind: TrackEventKind<'a>,
}

impl<'a> TrackInner<'a> {
	pub fn new(data: Vec<TrackEvent<'a>>) -> Self {
		Self {
			data,
			i: 0,
			last_time: 0,
		}
	}

	pub fn peek(&self) -> Option<InnerEvent<'a>> {
		self.data.get(self.i).map(|event| InnerEvent {
			t: self.last_time + event.delta.as_int(),
			kind: event.kind,
		})
	}
}

impl<'a> Iterator for TrackInner<'a> {
	type Item = InnerEvent<'a>;

	fn next(&mut self) -> Option<InnerEvent<'a>> {
		self.peek().inspect(|event| {
			self.last_time = event.t;
			self.i += 1;
		})
	}
}

/// A simplified view of a MIDI track, with the following changes:
/// - Absolute timestamps for events.
/// - Only note on/off events.
#[derive(Debug)]
pub struct Track<'a>(TrackInner<'a>);

pub struct Event {
	pub t: u32,
	pub channel: u8,
	pub key: u8,
	pub kind: EventKind,
}

pub enum EventKind {
	NoteOn { vel: NonZeroU8 },
	NoteOff,
}

impl<'a> Track<'a> {
	pub fn new(data: Vec<TrackEvent<'a>>) -> Self {
		Self(TrackInner::new(data))
	}

	pub fn peek(&mut self) -> Option<Event> {
		loop {
			let inner = self.0.peek()?;

			let TrackEventKind::Midi { channel, message } = inner.kind else {
				_ = self.0.next();
				continue;
			};

			let (key, vel) = match message {
				MidiMessage::NoteOn { key, vel } => (key, vel.as_int()),
				MidiMessage::NoteOff { key, vel: _ } => (key, 0),
				_ => {
					_ = self.0.next();
					continue;
				}
			};

			let kind = NonZeroU8::new(vel).map_or(EventKind::NoteOff, |vel| EventKind::NoteOn { vel });

			let ev = Event {
				t: inner.t,
				channel: channel.as_int(),
				key: key.as_int(),
				kind,
			};

			return Some(ev);
		}
	}
}

impl Iterator for Track<'_> {
	type Item = Event;

	fn next(&mut self) -> Option<Event> {
		self.peek().inspect(|_| {
			_ = self.0.next();
		})
	}
}
