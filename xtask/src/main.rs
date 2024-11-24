
use bindgen::CodegenConfig;

use std::fs::File;
use std::path::{Path, PathBuf};
use std::ffi::OsStr;

use clap::{Arg, App, SubCommand};

use base64::{decode, DecodeError};
use std::io::{self, Read, Write};
use walkdir::WalkDir;

const ATARI_ROMS_URL: &'static str = "https://gist.githubusercontent.com/jjshoots/61b22aefce4456920ba99f2c36906eda/raw/00046ac3403768bfe45857610a3d333b8e35e026/Roms.tar.gz.b64";
const ATARI_B64_TAR_FILENAME: &'static str = "Roms.tar.gz.b64";
const ATARI_TAR_FILENAME: &'static str = "Roms.tar.gz";

const XTASK_PREFIX: &'static str = "\x1B[1m\x1B[32m       xtask\x1B[0m ";
const ERROR_PREFIX: &'static str = "\x1B[1m\x1B[31merror\x1B[37m:\x1B[0m ";

fn main() {
	let mut app = App::new("ale-xtask")
		.version("0.1.0")
		.about("Build runner for the ale project")
		.author("Callum Tolley")
		.subcommand(SubCommand::with_name("gen-bindings")
			.about("Generate Arcade Learning Environment bindings"))
		.subcommand(SubCommand::with_name("download-roms")
			.about("Download builtin Atari ROMs, and place in the roms/ folder"))
		.subcommand(SubCommand::with_name("clean")
			.about("Remove the target directories")
			.arg(Arg::with_name("all")
				.long("all")
				.help("Remove the xtask target directory")));

	let matches = app.clone().get_matches();

	if let Some(_) = matches.subcommand_matches("download-roms") {
		eprintln!("{}download-roms", XTASK_PREFIX);
		run_download_roms();

	} else if let Some(matches) = matches.subcommand_matches("clean") {
		eprintln!("{}clean", XTASK_PREFIX);
		let mut rets = vec![
			run_rmdir(project_root().join("target"), false),
		];
		if matches.is_present("all") {
			rets.push(run_rmdir(project_root().join("xtask").join("target"), false));
		}
		if rets.iter().any(|r| r.is_err()) {
			std::process::exit(1);
		}
	} else {
		eprintln!("{}no subcommand specified", ERROR_PREFIX);
		app.print_help().expect("Failed to print help");
	}
}

fn run_download_roms() -> Result<&'static str, io::Error> {
	let dir = tempdir::TempDir::new("ale-xtask").expect("failed to generate temp directory");
	let b64_tar_path = dir.path().join(ATARI_B64_TAR_FILENAME);
	let tar_path = dir.path().join(ATARI_TAR_FILENAME);
	let extract_dir = dir.path().join("extract");



	println!("{:?}", b64_tar_path);

	run_download(ATARI_ROMS_URL, &b64_tar_path);
	
	let mut b64_file = File::open(&b64_tar_path)?;
	let mut content = String::new();
	b64_file.read_to_string(&mut content)?;
	let data: String = content.chars().filter(|&c| c != '\n' && c != '\r' && !c.is_whitespace()).collect();

	match decode(&data) {
		Ok(decoded_data) => {
				let mut file = File::create(&tar_path)?;
				file.write_all(&decoded_data)?;
		}
		Err(e) => eprintln!("Error decoding Base64 content: {}", e),
	}


	run_extract(&tar_path, &extract_dir);
	let roms_dir = project_root().join("roms");
	std::fs::create_dir_all(&roms_dir).expect("failed to create roms dir");

	for entry in WalkDir::new(&extract_dir).into_iter().filter_map(Result::ok) {
		let path = entry.path();
		if path.is_file() && path.extension() == Some(std::ffi::OsStr::new("bin")) {
			std::fs::copy(entry.path(), roms_dir.join(entry.path().file_name().unwrap()));
		}
	}
	
	Ok("")
}

fn run_download(url: &str, dst: &Path) {
	eprintln!("{}download {}", XTASK_PREFIX, url);
	let mut out = File::create(&dst).expect("failed to create dst file");
	let mut res = match reqwest::blocking::get(url) {
		Err(e) => {
			eprintln!("{}failed to download {}: {}", ERROR_PREFIX, url, e);
			::std::process::exit(1);
		},
		Ok(r) => r,
	};

	if !res.status().is_success() {
		eprintln!("{}failed to download {}: status is {:?}", ERROR_PREFIX, url, res.status());
		::std::process::exit(1);
	}

	if let Err(e) = res.copy_to(&mut out) {
		eprintln!("{}failed to download {}: {}", ERROR_PREFIX, url, e);
		::std::process::exit(1);
	}
}

fn run_extract(tar_path: &Path, extract_dir: &Path) {
	eprintln!("{}extract {} to {}", XTASK_PREFIX, tar_path.file_name().unwrap_or(OsStr::new("")).to_string_lossy(), extract_dir.display());
	std::fs::create_dir_all(&extract_dir).expect("failed to create extract dir");

	let tar_gz = File::open(tar_path).expect("failed to open tar.gz");
	let tar = flate2::read::GzDecoder::new(tar_gz);
	let mut archive = tar::Archive::new(tar);
	archive.unpack(extract_dir).expect("failed to extract tar.gz");
}

fn run_rmdir(dir: impl AsRef<Path>, error_fail: bool) -> Result<(), ()> {
	let dir = dir.as_ref();
	eprintln!("{}delete directory {}", XTASK_PREFIX, dir.display());
	if let Err(e) = fs_extra::dir::remove(dir) {
		eprintln!("{}failed to delete directory: {}", ERROR_PREFIX, e);
		if error_fail {
			std::process::exit(1);
		}
		Err(())
	} else {
		Ok(())
	}
}

fn project_root() -> PathBuf {
	Path::new(&env!("CARGO_MANIFEST_DIR"))
		.ancestors()
		.nth(1)
		.unwrap()
		.to_path_buf()
}
