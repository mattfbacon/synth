use std::f32::consts::TAU;
use std::num::NonZeroU8;

use crate::SAMPLE_RATE;
use crate::wave::Wave;

const FADE_SAMPLES: u8 = 20;

pub type Key = u8;

pub fn key_to_freq(key: Key) -> f32 {
	// MIDI is based on C while our math is based on A, so the translation is 5 half steps down.
	const GLOBAL_OFFSET: i16 = -5;
	const A: f32 = 415.3;
	// Neidhart Big City, https://harpsichords.pbworks.com/f/Neidhardt_1732_Charts.pdf
	const D: [f32; 12] = [0.0, 5.9, 2.0, 5.9, 2.0, 2.0, 3.9, 0.0, 5.9, 2.0, 3.9, 2.0];
	let offset = i16::from(key) - 66i16 + GLOBAL_OFFSET;
	let i = offset.rem_euclid(12) as usize;
	A * 2f32.powf((f32::from(offset) + D[i] / 100.0) / 12.0)
}

#[derive(Debug, Clone)]
pub struct Note {
	key: Key,
	vel: NonZeroU8,
	t: f32,
	sample_t: u32,
	end_in: Option<NonZeroU8>,
}

impl Note {
	fn step_t(&mut self) {
		const TREM_PERIOD_S: f32 = 0.3;
		// Multiplied with `freq`
		const TREM_AMPLITUDE_FACTOR: f32 = 2.5 / 440.0;
		const TREM_START_S: f32 = 1.0;
		const TREM_ATTACK_S: f32 = 0.5;

		let freq = key_to_freq(self.key);

		let secs_t = self.sample_t as f32 / SAMPLE_RATE as f32;
		let trem = (secs_t / TREM_PERIOD_S * TAU).sin() * TREM_AMPLITUDE_FACTOR * freq;
		let trem_strength = ((secs_t - TREM_START_S) / TREM_ATTACK_S).clamp(0.0, 1.0);

		self.t += (freq + trem * trem_strength) / SAMPLE_RATE as f32;

		if self.t >= 1.0 {
			self.t -= 1.0;
		}

		self.sample_t += 1;
	}

	pub fn new(key: Key, vel: NonZeroU8) -> Self {
		Self {
			key,
			vel,
			t: 0.0,
			sample_t: 0,
			end_in: None,
		}
	}

	/// Returns the generated sample.
	/// If `None`, the note has ended and must be removed.
	pub fn next(&mut self, wave: Wave) -> Option<f32> {
		let volume = if let Some(end_in) = &mut self.end_in {
			// Decay.
			let volume = f32::from(end_in.get()) / f32::from(FADE_SAMPLES);
			*end_in = (end_in.get() - 1).try_into().ok()?;
			volume
		} else if self.sample_t < u32::from(FADE_SAMPLES) * 3 / 2 {
			// Attack. `* 1.5` creates an attack peak 50% louder than the sustain volume.
			self.sample_t as f32 / f32::from(FADE_SAMPLES)
		} else {
			1.0
		};
		let vel_volume = f32::from(self.vel.get()) / 127.0;
		let sample = wave.synth(self.t) * volume * vel_volume;

		self.step_t();

		Some(sample)
	}

	pub fn end(&mut self) {
		if self.end_in.is_some() {
			return;
		}

		// Avoid volume jump if `end` is called during attack ramp.
		let end_in = u32::from(FADE_SAMPLES).min(self.sample_t).max(1);
		self.end_in = Some(u8::try_from(end_in).unwrap().try_into().unwrap());
	}

	pub fn reset(&mut self, vel: NonZeroU8) {
		*self = Self::new(self.key, vel);
	}

	pub fn key(&self) -> Key {
		self.key
	}
}
