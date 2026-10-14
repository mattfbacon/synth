use serde::Deserialize;

use crate::wave::Wave;

#[derive(Deserialize)]
pub struct SynthConfig {
	pub pan: f32,
	pub volume: f32,
	pub wave: Wave,
}

#[derive(Deserialize)]
pub struct Config {
	pub name: String,
	pub samples_per_tick: u32,
	pub track: Vec<SynthConfig>,
}
