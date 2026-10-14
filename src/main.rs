#![deny(
	absolute_paths_not_starting_with_crate,
	keyword_idents,
	macro_use_extern_crate,
	meta_variable_misuse,
	missing_abi,
	missing_copy_implementations,
	non_ascii_idents,
	nonstandard_style,
	noop_method_call,
	rust_2018_idioms,
	unused_qualifications
)]
#![warn(clippy::pedantic)]
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
#![forbid(unsafe_code)]

use std::path::PathBuf;

use argh::FromArgs;
use cpal::traits::{DeviceTrait as _, HostTrait as _, StreamTrait as _};
use cpal::{BufferSize, StreamConfig};
use midly::{Format, Smf};

use crate::config::Config;
use crate::midi::{EndOfFile, MidiPlayer};

mod config;
mod midi;
mod note;
mod track;
mod video;
mod wave;

const SAMPLE_RATE: u32 = 48_000;

fn print_name(name: &str) {
	text2art::Printer::with_font(text2art::Font::from_basic(text2art::BasicFonts::Big).unwrap())
		.print_to_stdio(name)
		.unwrap();
	println!("\n\n");
}

/// Play a midi song.
#[derive(FromArgs)]
struct Args {
	/// print visualization to stdout
	#[argh(switch, short = 'p')]
	print: bool,
	/// generate an MP4 video file instead of playing live
	#[argh(option, short = 'o')]
	output: Option<PathBuf>,
	#[argh(positional)]
	song_name: String,
}

fn main() {
	let args: Args = argh::from_env();
	let song_name = args.song_name;

	let input: &'static [u8] = std::fs::read(format!("{song_name}.mid")).unwrap().leak();

	let smf = Smf::parse(input).unwrap();
	assert_eq!(smf.header.format, Format::Parallel);

	let conf: Config =
		toml::from_str(&std::fs::read_to_string(format!("{song_name}.toml")).unwrap()).unwrap();
	let conf: &'static Config = Box::leak(Box::new(conf));

	if args.print {
		print_name(&conf.name);
	}

	let mut player = MidiPlayer::new(smf, conf, args.print);
	if let Err(EndOfFile) = player.jump_to_next_note() {
		eprintln!("empty file - no notes - exiting");
		return;
	}

	if let Some(output) = args.output {
		video::generate(player, &output);
		return;
	}

	let host = cpal::default_host();
	let device = host.default_output_device().unwrap();
	let config = StreamConfig {
		channels: 2,
		sample_rate: SAMPLE_RATE,
		buffer_size: BufferSize::Default,
	};
	let stream = device
		.build_output_stream(
			config,
			move |samples: &mut [f32], _| {
				for out_sample in samples.as_chunks_mut::<2>().0 {
					let Some(sample) = player.next() else {
						eprintln!("end");
						std::process::exit(0);
					};
					*out_sample = sample;
				}
			},
			|error| panic!("cpal stream error: {error:?}"),
			None,
		)
		.unwrap();
	stream.play().unwrap();
	loop {
		std::thread::park();
	}
}
