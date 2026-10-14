//! The core of the synth is the wave abstraction.
//! Rather than dealing with frequencies and sample rates here, we use an abstract `t` input which indicates the position in the wave's period.
//! Thus, to generate a signal at `freq` hz, increase `t` by `freq / SAMPLE_RATE` for each sample.
//! Make sure `t` stays in `0.0..1.0` -- some waves will misbehave otherwise.

use std::f32::consts::TAU;

use serde::Deserialize;

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum Wave {
	Square,
	Saw,
	Tri,
	Sine,
	/// Used to indicate that a track has no notes; panics if played.
	None,
}

impl Wave {
	// t in 0.0..1.0, f(0.0) == f(1.0)
	pub fn synth(self, t: f32) -> f32 {
		match self {
			Self::Square => {
				if t >= 0.5 {
					1.0
				} else {
					-1.0
				}
			}
			Self::Saw => t * 2.0 - 1.0,
			Self::Tri => (if t >= 0.5 { 1.0 - t } else { t }) * 4.0 - 1.0,
			Self::Sine => (t * TAU).sin(),
			Self::None => panic!("played none"),
		}
	}
}
