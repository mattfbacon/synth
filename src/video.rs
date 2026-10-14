use std::collections::VecDeque;
use std::io::Write as _;
use std::path::Path;
use std::process::Stdio;

use crate::SAMPLE_RATE;
use crate::midi::{ActiveSet, MidiPlayer};

const VIDEO_WIDTH: u32 = 1920;
const VIDEO_HEIGHT: u32 = 1080;
const NOTE_WIDTH: u32 = VIDEO_WIDTH / 128;
const NOTE_HEIGHT: u32 = 15;
const FRAME_RATE: u32 = 15;

const MAX_ROWS: usize = ((VIDEO_HEIGHT - 150) / NOTE_HEIGHT) as usize;

fn write_frame(name: &str, svg_notes: &VecDeque<ActiveSet>, out_path: &Path) {
	let mut out = std::io::BufWriter::new(std::fs::File::create(out_path).unwrap());
	writeln!(
		&mut out,
		r##"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<svg width="{VIDEO_WIDTH}" height="{VIDEO_HEIGHT}" viewBox="0 0 {VIDEO_WIDTH} {VIDEO_HEIGHT}">
	<rect fill="#fef8ec" x="0" y="0" width="{VIDEO_WIDTH}" height="{VIDEO_HEIGHT}" />
	<text style="font-size:80px;font-family:'Crimson Text',serif;text-align:center;text-anchor:middle;fill:#004d57" x="960" y="100">{name}</text>"##,
	).unwrap();
	for (i, notes) in svg_notes.iter().rev().enumerate() {
		let y = VIDEO_HEIGHT - (i as u32 + 1) * NOTE_HEIGHT;
		for (note, &voice) in notes.iter().enumerate() {
			let x = note as u32 * NOTE_WIDTH;
			if voice == 0 {
				continue;
			}
			let color = ["8ca6a6", "0fa3ff", "ff6b9f", "ff8c00", "00d17a", "e34e1c"][usize::from(voice)];
			writeln!(
				&mut out,
				r##"	<rect fill="#{color}" x="{x}" y="{y}" width="{NOTE_WIDTH}" height="{NOTE_HEIGHT}"/>"##,
			)
			.unwrap();
		}
	}
	writeln!(&mut out, "</svg>").unwrap();
}

pub fn generate(mut midi: MidiPlayer<'_>, out_path: &Path) {
	let out_path = std::path::absolute(out_path).unwrap();

	let samples_per_frame = SAMPLE_RATE / FRAME_RATE;
	// Actual frame rate may be different due to being sample-based and rounded.
	// This needs to be as precise as possible to avoid audio desyncing, so recalculate.
	let frame_rate = SAMPLE_RATE as f32 / samples_per_frame as f32;

	let temp_dir = temp_dir::TempDir::new().unwrap();
	std::env::set_current_dir(temp_dir.path()).unwrap();

	let mut svg_notes = VecDeque::with_capacity(MAX_ROWS);

	let audio_spec = hound::WavSpec {
		channels: 2,
		sample_rate: SAMPLE_RATE,
		bits_per_sample: 32,
		sample_format: hound::SampleFormat::Float,
	};
	let mut audio_writer = hound::WavWriter::create("audio.wav", audio_spec).unwrap();

	let mut svg_i = 0;
	let mut sample_i = 0;

	while let Some(sample) = midi.next() {
		for sample in sample {
			audio_writer.write_sample(sample).unwrap();
		}
		if sample_i == 0 {
			write_frame(
				&midi.conf().name,
				&svg_notes,
				format!("{svg_i}.svg").as_ref(),
			);
			svg_i += 1;
		}
		sample_i += 1;
		if sample_i == samples_per_frame {
			if svg_notes.len() >= MAX_ROWS {
				svg_notes.pop_front();
			}
			svg_notes.push_back(midi.active_set());

			sample_i = 0;
		}
	}

	audio_writer.finalize().unwrap();

	std::process::Command::new("ffmpeg")
		.args([
			"-hide_banner",
			"-framerate",
			&frame_rate.to_string(),
			"-i",
			"%d.svg",
			"-i",
			"audio.wav",
		])
		.arg(out_path)
		// For overwrite prompt.
		.stdin(Stdio::inherit())
		.spawn()
		.unwrap()
		.wait()
		.unwrap();
}
