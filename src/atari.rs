//! Rust encapsulation of the [Arcade Learning Environment](https://github.com/mgbellemare/Arcade-Learning-Environment).
//!
//! The main use of the ALE is running Atari 2600 games. An example for how to play breakout is included in the library. <kbd>Space</kbd> to start.
//! ```sh
//! git clone --recursive https://github.com/trolleyman/ale-rs.git
//! cd ale-rs
//! cargo xtask download-roms  # Breakout ROM needs to be downloaded from a third-party source
//! cargo run --release --example breakout
//! ```
//!
//! # Requirements
//! This library requires the same dependencies as the [cmake-rs](https://github.com/alexcrichton/cmake-rs) library. In other words, [CMake](https://cmake.org/) needs to be installed.
//!
//! # Unsafety
//! Generally this libarary has tried to encapsulate and minimize unsafety, but there could still be some pain points that I've missed (especially regarding C++ exceptions). Be sure to report an issue if this is the case!

use std::convert::TryInto;
use std::ffi::{CStr, CString};
use std::io;
use std::os::raw::c_int;
use std::ptr::null_mut;

/// Interface to the Arcade Learning Environment emulator
pub struct Ale {
	ptr: *mut ale_sys::ALEInterface,
	action_set: Vec<i32>,
	screen_data: Vec<u8>,
	screen_size: (u16, u16),
	score: i32,
	max_frames: u32,
	lives: u8,
}

unsafe impl Send for Ale {

}

unsafe impl Sync for Ale {

}

impl Ale {
	/// Creates a new interface to the Arcade Learning Environment, i.e. a new emulator instance.
	pub fn new(max_frames: u32) -> Ale {
		let ptr = unsafe { ale_sys::ALE_new() };
		assert!(ptr != null_mut());
		let screen_size = unsafe{(
			ale_sys::getScreenHeight(ptr) as u16, 
			ale_sys::getScreenWidth(ptr) as u16)};
		let action_set = unsafe {
			let n = ale_sys::getLegalActionSize(ptr);
			let mut set = vec![0i32; n as usize];
			ale_sys::getLegalActionSet(ptr, set.as_mut_ptr());
			set
		};
		let screen_data = vec![0u8; (screen_size.0 * screen_size.1 * 3) as usize];
		let score = 0;
		let lives = 0;
		Ale { ptr, action_set, screen_data, screen_size, score, max_frames, lives}
	}



	/// Applies an action to the game and returns the reward.
	///
	/// It is the user's responsibility to check if the game has ended and reset
	/// when necessary - this method will keep pressing buttons on the game over screen.
	pub fn act(&mut self, action: u8) -> (i32, bool, bool) {
		unsafe {
			let a = self.action_set.iter().nth(action as usize).unwrap();
			let r = ale_sys::act(self.ptr, *a);
			self.score += r;
			ale_sys::getScreenRGB(self.ptr, self.screen_data.as_mut_ptr());
			let live_loss = (self.current_lives() as u8) < self.lives;
			self.lives = self.current_lives() as u8;
			let truncation = self.episode_frame_number() as u32 >= self.max_frames;
			(r, live_loss, truncation)
		}
	}

	/// Resets the game, but not the full system.
	pub fn reset_game(&mut self) {
		unsafe {
			ale_sys::reset_game(self.ptr);
			self.score = 0;
			self.lives = self.current_lives() as u8;
			ale_sys::getScreenRGB(self.ptr, self.screen_data.as_mut_ptr());
		}
	}

	pub fn action_dim(&mut self) -> u8 {
		self.action_set.len() as u8
	}

	pub fn screen(&self) -> Vec<u8> {
		self.screen_data.clone()
	}

	/// Indicates if the game has ended.
	pub fn is_game_over(&mut self) -> bool {
		unsafe { ale_sys::game_over(self.ptr) }
	}

		// pub fn getString(ale: *mut ALEInterface, key: *const c_char) -> *const c_char; // TODO

	// Gets the value of an integer setting.
	pub fn get_int(&mut self, key: &str) -> i32 {
		let c_key = CString::new(key).unwrap();
		unsafe { ale_sys::getInt(self.ptr, c_key.as_ptr()) }
	}

	// Gets the value of a bool setting.
	pub fn get_bool(&mut self, key: &str) -> bool {
		let c_key = CString::new(key).unwrap();
		unsafe { ale_sys::getBool(self.ptr, c_key.as_ptr()) }
	}

	// Gets the value of a float setting.
	pub fn get_float(&mut self, key: &str) -> f32 {
		let c_key = CString::new(key).unwrap();
		unsafe { ale_sys::getFloat(self.ptr, c_key.as_ptr()) }
	}

	// Sets the value of a string setting.
	pub fn set_string(&mut self, key: &str, value: &str) {
		let c_key = CString::new(key).unwrap();
		let c_value = CString::new(value).unwrap();
		unsafe {
			ale_sys::setString(self.ptr, c_key.as_ptr(), c_value.as_ptr());
		}
	}

	// Sets the value of a bool setting.
	pub fn set_bool(&mut self, key: &str, value: bool) {
		let c_key = CString::new(key).unwrap();
		unsafe {
			ale_sys::setBool(self.ptr, c_key.as_ptr(), value);
		}
	}

	// Sets the value of an integer setting.
	pub fn set_int(&mut self, key: &str, value: i32) {
		let c_key = CString::new(key).unwrap();
		unsafe {
			ale_sys::setInt(self.ptr, c_key.as_ptr(), value);
		}
	}

	// Sets the value of a float setting.
	pub fn set_float(&mut self, key: &str, value: f32) {
		let c_key = CString::new(key).unwrap();
		unsafe {
			ale_sys::setFloat(self.ptr, c_key.as_ptr(), value);
		}
	}

	/// Resets the Atari and loads a bundled game.
	///
	/// After this call the game should be ready to play. This is necessary after changing a
	/// setting for the setting to take effect.
	///
	/// Returns an error if there was an IO exception when saving the bundled ROM to a temporary directory.
	///
	/// # Examples
	/// ```
	/// # use ale::{Ale, BundledRom};
	/// let mut ale = Ale::new();
	/// ale.load_rom(BundledRom::Breakout);
	/// ale.act(1);
	/// assert_eq!(ale.is_game_over(), false);
	/// ```
	pub fn load_rom(&mut self, rom: BundledRom) -> io::Result<()> {
		// Save ROM to temp dir
		let dir = tempdir::TempDir::new("ale-rs")?;
		let rom_path = dir.path().join(rom.filename());
		std::fs::write(&rom_path, rom.data())?;

		// Call load_rom_file
		let rom_path_string = rom_path.to_string_lossy().to_string();
		let rom_path_c_str = CString::new(rom_path_string).expect("Invalid path");
		self.load_rom_file(&rom_path_c_str);
		Ok(())
	}

	/// Resets the Atari and loads a game from the file specified.
	///
	/// After this call the game should be ready to play. This is necessary after changing a
	/// setting for the setting to take effect.
	pub fn load_rom_file(&mut self, rom_file: &CStr) {
		unsafe {
			ale_sys::loadROM(self.ptr, rom_file.as_ptr());
		}
	}
	
	/// Returns the vector of modes available for the current game.
	///
	/// This should be called only after the rom is loaded.
	pub fn available_modes(&mut self) -> Vec<i32> {
		let size = unsafe { ale_sys::getAvailableModesSize(self.ptr) };
		assert!(size >= 0);
		let mut available_modes = vec![0; size as usize];
		unsafe {
			ale_sys::getAvailableModes(self.ptr, available_modes.as_mut_ptr());
		}
		return available_modes;
	}

	/// Sets the mode of the game.
	///
	/// This should be called only after the rom is loaded.
	///
	/// # Panics
	/// If the mode is invalid.
	pub fn set_mode(&mut self, mode: i32) {
		assert!(self.available_modes().contains(&mode), "Invalid mode: {}", mode);
		unsafe {
			ale_sys::setMode(self.ptr, mode);
		}
	}

	/// Returns the vector of difficulties available for the current game.
	///
	/// This should be called only after the rom is loaded.
	///
	/// Notice that there are 2 levers, the right and left switches. They are not tied to any specific player. In Venture, for example, we have the following interpretation for the difficulties:
	///
	/// | Skill Level | Switch Setting |
	/// |-------------|----------------|
	/// | 1           | left B/right B |
	/// | 2           | left B/right A |
	/// | 3           | left A/right B |
	/// | 4           | left A/right A |
	pub fn available_difficulties(&mut self) -> Vec<i32> {
		let size = unsafe { ale_sys::getAvailableDifficultiesSize(self.ptr) };
		assert!(size >= 0);
		let mut available_difficulties = vec![0; size as usize];
		unsafe {
			ale_sys::getAvailableDifficulties(self.ptr, available_difficulties.as_mut_ptr());
		}
		return available_difficulties;
	}

	/// Sets the difficulty of the game.
	///
	/// This should be called only after the rom is loaded.
	///
	/// # Panics
	/// If the difficulty is not a valid difficulty
	pub fn set_difficulty(&mut self, difficulty: i32) {
		assert!(self.available_difficulties().contains(&difficulty), "Invalid difficulty: {}", difficulty);
		unsafe {
			ale_sys::setDifficulty(self.ptr, difficulty);
		}
	}

	/// Returns the vector of legal actions. This should be called only after the ROM is loaded.
	pub fn legal_action_set(&mut self) -> Vec<i32> {
		let size = unsafe { ale_sys::getLegalActionSize(self.ptr) };
		assert!(size >= 0);
		let mut legal_actions = vec![0; size as usize];
		unsafe {
			ale_sys::getLegalActionSet(self.ptr, legal_actions.as_mut_ptr());
		}
		return legal_actions;
	}

	/// Returns the vector of the minimal set of actions needed to play the game.
	pub fn minimal_action_set(&mut self) -> Vec<i32> {
		let size = unsafe { ale_sys::getMinimalActionSize(self.ptr) };
		assert!(size >= 0);
		let mut minimal_actions = vec![0; size as usize];
		unsafe {
			ale_sys::getMinimalActionSet(self.ptr, minimal_actions.as_mut_ptr());
		}
		return minimal_actions;
	}

	/// Returns the frame number since the loading of the ROM.
	pub fn frame_number(&mut self) -> i32 {
		unsafe { ale_sys::getFrameNumber(self.ptr) as i32 }
	}

	/// Returns the remaining number of lives.
	pub fn current_lives(&mut self) -> i32 {
		unsafe { ale_sys::lives(self.ptr) }
	}

	/// Returns the frame number since the start of the current episode.
	pub fn episode_frame_number(&mut self) -> i32 {
		unsafe { ale_sys::getEpisodeFrameNumber(self.ptr) }
	}

	/// Writes the emulator's RAM contents to the buffer provided.
	///
	/// # Panics
	/// If the buffer is smaller than what [`Ale::ram_size()`] returns.
	pub fn get_ram(&mut self, ram: &mut [u8]) {
		assert!(ram.len() >= self.ram_size());
		unsafe {
			ale_sys::getRAM(self.ptr, ram.as_mut_ptr());
		}
	}

	/// Get the size of the emulator's RAM, in bytes.
	pub fn ram_size(&mut self) -> usize {
		unsafe { ale_sys::getRAMSize(self.ptr) }.try_into().expect("invalid size")
	}

	/// Get the scren's width in pixels.
	pub fn screen_width(&mut self) -> usize {
		unsafe { ale_sys::getScreenWidth(self.ptr) }.try_into().expect("invalid size")
	}

	/// Get the scren's height in pixels.
	pub fn screen_height(&mut self) -> usize {
		unsafe { ale_sys::getScreenHeight(self.ptr) }.try_into().expect("invalid size")
	}

	/// Writes the screen's data to the buffer provided, in RGB format.
	///
	/// Pixel value at `x,y` is equal to `scren_data[y * screen_width() + x]`.
	///
	/// # Panics
	/// If the buffer is smaller than `screen_width() * screen_height() * 3`.
	pub fn get_screen_rgb(&mut self, screen_data: &mut [u8]) {
		assert!(screen_data.len() >= self.screen_width() * self.screen_height() * 3);
		unsafe {
			ale_sys::getScreenRGB(self.ptr, screen_data.as_mut_ptr());
		}
	}

	/// Writes the screen's data to the buffer provided, in grayscale format, where `0 = black` and `255 = white`.
	///
	/// Pixel value at `x,y` is equal to `scren_data[y * screen_width() + x]`.
	///
	/// # Panics
	/// If the buffer is smaller than `screen_width() * screen_height()`.
	pub fn get_screen_grayscale(&mut self, screen_data: &mut [u8]) {
		assert!(screen_data.len() >= self.screen_width() * self.screen_height());
		unsafe {
			ale_sys::getScreenGrayscale(self.ptr, screen_data.as_mut_ptr());
		}
	}

	/// Save the state of the system, to be restored using [`Ale::load_state`].
	pub fn save_state(&mut self) {
		unsafe {
			ale_sys::saveState(self.ptr);
		}
	}

	/// Loads the state of the system that was saved by [`Ale::save_state`].
	pub fn load_state(&mut self) {
		unsafe {
			ale_sys::loadState(self.ptr);
		}
	}

	/// This makes a copy of the environment state. This copy does *not* include pseudorandomness, making it suitable for planning purposes. By contrast, see [`Ale::clone_system_state()`].
	pub fn clone_state(&mut self) -> AleState {
		AleState { ptr: unsafe { ale_sys::cloneState(self.ptr) } }
	}

	/// Reverse operation of [`Ale::clone_state`]. This does not restore pseudorandomness, so that repeated
	/// calls to [`Ale::restore_state`] in the stochastic controls setting will not lead to the same outcomes.
	///
	/// By contrast, see [`Ale::restore_system_state`].
	pub fn restore_state(&mut self, state: &AleState) {
		unsafe {
			ale_sys::restoreState(self.ptr, state.ptr);
		}
	}

	/// This makes a copy of the system & environment state, suitable for serialization. This includes pseudorandomness and so is *not* suitable for planning purposes.
	pub fn clone_system_state(&mut self) -> AleState {
		AleState { ptr: unsafe { ale_sys::cloneSystemState(self.ptr) } }
	}

	/// Reverse operation of [`Ale::clone_system_state`].
	pub fn restore_system_state(&mut self, state: &AleState) {
		unsafe {
			ale_sys::restoreSystemState(self.ptr, state.ptr);
		}
	}

	/// Save the current screen as a png file
	///
	/// # Unsafety
	/// I am not sure, but this function may trigger undefined behaviour when a C++ exception is triggered.
	///
	/// To be safe, this function is marked as unsafe.
	pub unsafe fn save_screen_png(&mut self, filename: &CStr) {
		ale_sys::saveScreenPNG(self.ptr, filename.as_ptr());
	}

	/// Set logger mode
	pub fn set_logger_mode(mode: LoggerMode) {
		unsafe {
			ale_sys::setLoggerMode(mode as c_int);
		}
	}
}
impl Drop for Ale {
	fn drop(&mut self) {
		unsafe {
			let ptr = self.ptr;
			self.ptr = std::ptr::null_mut();
			ale_sys::ALE_del(ptr);
		}
	}
}

/// State of the ALE
///
/// Used mainly by [`Ale::clone_state`] & [`Ale::restore_state`] to save the emulator's state, and restore it at a later point.
pub struct AleState {
	ptr: *mut ale_sys::ALEState,
}
impl AleState {
	/// Encodes the state as a raw bytestream.
	///
	/// # Panics
	/// If the length of `buf` is not large enough. Use [`AleState::encode_state_len`] to get the needed length.
	pub fn encode_state(&self, buf: &mut [u8]) {
		assert!(
			buf.len() >= self.encode_state_len(),
			"Buffer not long enough to store encoded state. Expected {}, got {}",
			self.encode_state_len(),
			buf.len()
		);
		unsafe {
			ale_sys::encodeState(self.ptr, buf.as_mut_ptr() as *mut _, buf.len() as c_int);
		}
	}

	/// Returns the length of the buffer needed to encode the state.
	///
	/// # Panics
	/// If the C API returns a negative size.
	pub fn encode_state_len(&self) -> usize {
		let size = unsafe { ale_sys::encodeStateLen(self.ptr) };
		assert!(size >= 0, "Invalid size: {}", size);
		size as usize
	}

	/// Decode state from a raw bytestream.
	///
	/// # Panics
	/// If the serialized length is too long to fit into a C integer.
	pub fn decode_state(serialized: &[u8]) -> AleState {
		let len: c_int = serialized.len().try_into().expect("Length too long");
		// TODO: Exceptions
		AleState { ptr: unsafe { ale_sys::decodeState(serialized.as_ptr() as *const _, len) } }
	}
}
impl Drop for AleState {
	fn drop(&mut self) {
		unsafe {
			let ptr = self.ptr;
			self.ptr = std::ptr::null_mut();
			ale_sys::deleteState(ptr);
		}
	}
}

pub enum LoggerMode {
	Info = 0,
	Warning = 1,
	Error = 2,
}

/// Enum of ROMs that come bundled with the libarary.
///
/// Note: Commented out ROMs are supported, but not bundled.
pub enum BundledRom {
	Adventure,
	AirRaid,
	Alien,
	Amidar,
	Assault,
	Asterix,
	Asteroids,
	Atlantis,
	Atlantis2,
	Backgammon,
	BankHeist,
	BasicMath,
	BattleZone,
	BeamRider,
	Berzerk,
	Blackjack,
	Bowling,
	Boxing,
	Breakout,
	Carnival,
	Casino,
	Centipede,
	ChopperCommand,
	Combat,
	CrazyClimber,
	Crossbow,
	Darkchambers,
	Defender,
	DemonAttack,
	DonkeyKong,
	DoubleDunk,
	Earthworld,
	ElevatorAction,
	Enduro,
	Entombed,
	Et,
	FishingDerby,
	FlagCapture,
	Freeway,
	Frogger,
	Frostbite,
	Galaxian,
	Gopher,
	Gravitar,
	Hangman,
	HauntedHouse,
	Hero,
	HumanCannonball,
	IceHockey,
	Jamesbond,
	JourneyEscape,
	Joust,
	Kaboom,
	Kangaroo,
	KeystoneKapers,
	KingKong,
	Klax,
	Koolaid,
	Krull,
	KungFuMaster,
	LaserGates,
	LostLuggage,
	MarioBros,
	MazeCraze,
	MiniatureGolf,
	MontezumaRevenge,
	MrDo,
	MsPacman,
	NameThisGame,
	Othello,
	Pacman,
	Phoenix,
	Pitfall,
	Pitfall2,
	Pong,
	Pooyan,
	PrivateEye,
	Qbert,
	Riverraid,
	RoadRunner,
	Robotank,
	Seaquest,
	SirLancelot,
	Skiing,
	Solaris,
	SpaceInvaders,
	SpaceWar,
	StarGunner,
	Superman,
	Surround,
	Tennis,
	Tetris,
	TicTacToe3d,
	TimePilot,
	Trondead,
	Turmoil,
	Tutankham,
	UpNDown,
	Venture,
	VideoCheckers,
	VideoChess,
	VideoCube,
	VideoPinball,
	Warlords,
	WizardOfWor,
	WordZapper,
	YarsRevenge,
	Zaxxon,
}
impl BundledRom {
	/// Returns the filename that the ROM should be named, in order for the ALE to pick up on it and
	/// use the correct settings.
	pub fn filename(&self) -> &'static str {
		use BundledRom::*;
		match self {
			Adventure => "adventure.bin",
			AirRaid => "air_raid.bin",
			Alien => "alien.bin",
			Amidar => "amidar.bin",
			Assault => "assault.bin",
			Asterix => "asterix.bin",
			Asteroids => "asteroids.bin",
			Atlantis => "atlantis.bin",
			Atlantis2 => "atlantis2.bin",
			Backgammon => "backgammon.bin",
			BankHeist => "bank_heist.bin",
			BasicMath => "basic_math.bin",
			BattleZone => "battle_zone.bin",
			BeamRider => "beam_rider.bin",
			Berzerk => "berzerk.bin",
			Blackjack => "blackjack.bin",
			Bowling => "bowling.bin",
			Boxing => "boxing.bin",
			Breakout => "breakout.bin",
			Carnival => "carnival.bin",
			Casino => "casino.bin",
			Centipede => "centipede.bin",
			ChopperCommand => "chopper_command.bin",
			Combat => "combat.bin",
			CrazyClimber => "crazy_climber.bin",
			Crossbow => "crossbow.bin",
			Darkchambers => "darkchambers.bin",
			Defender => "defender.bin",
			DemonAttack => "demon_attack.bin",
			DonkeyKong => "donkey_kong.bin",
			DoubleDunk => "double_dunk.bin",
			Earthworld => "earthworld.bin",
			ElevatorAction => "elevator_action.bin",
			Enduro => "enduro.bin",
			Entombed => "entombed.bin",
			Et => "et.bin",
			FishingDerby => "fishing_derby.bin",
			FlagCapture => "flag_capture.bin",
			Freeway => "freeway.bin",
			Frogger => "frogger.bin",
			Frostbite => "frostbite.bin",
			Galaxian => "galaxian.bin",
			Gopher => "gopher.bin",
			Gravitar => "gravitar.bin",
			Hangman => "hangman.bin",
			HauntedHouse => "haunted_house.bin",
			Hero => "hero.bin",
			HumanCannonball => "human_cannonball.bin",
			IceHockey => "ice_hockey.bin",
			Jamesbond => "jamesbond.bin",
			JourneyEscape => "journey_escape.bin",
			Joust => "joust.bin",
			Kaboom => "kaboom.bin",
			Kangaroo => "kangaroo.bin",
			KeystoneKapers => "keystone_kapers.bin",
			KingKong => "king_kong.bin",
			Klax => "klax.bin",
			Koolaid => "koolaid.bin",
			Krull => "krull.bin",
			KungFuMaster => "kung_fu_master.bin",
			LaserGates => "laser_gates.bin",
			LostLuggage => "lost_luggage.bin",
			MarioBros => "mario_bros.bin",
			MazeCraze => "maze_craze.bin",
			MiniatureGolf => "miniature_golf.bin",
			MontezumaRevenge => "montezuma_revenge.bin",
			MrDo => "mr_do.bin",
			MsPacman => "ms_pacman.bin",
			NameThisGame => "name_this_game.bin",
			Othello => "othello.bin",
			Pacman => "pacman.bin",
			Phoenix => "phoenix.bin",
			Pitfall => "pitfall.bin",
			Pitfall2 => "pitfall2.bin",
			Pong => "pong.bin",
			Pooyan => "pooyan.bin",
			PrivateEye => "private_eye.bin",
			Qbert => "qbert.bin",
			Riverraid => "riverraid.bin",
			RoadRunner => "road_runner.bin",
			Robotank => "robotank.bin",
			Seaquest => "seaquest.bin",
			SirLancelot => "sir_lancelot.bin",
			Skiing => "skiing.bin",
			Solaris => "solaris.bin",
			SpaceInvaders => "space_invaders.bin",
			SpaceWar => "space_war.bin",
			StarGunner => "star_gunner.bin",
			Superman => "superman.bin",
			Surround => "surround.bin",
			Tennis => "tennis.bin",
			Tetris => "tetris.bin",
			TicTacToe3d => "tic_tac_toe_3d.bin",
			TimePilot => "time_pilot.bin",
			Trondead => "trondead.bin",
			Turmoil => "turmoil.bin",
			Tutankham => "tutankham.bin",
			UpNDown => "up_n_down.bin",
			Venture => "venture.bin",
			VideoCheckers => "video_checkers.bin",
			VideoChess => "video_chess.bin",
			VideoCube => "video_cube.bin",
			VideoPinball => "video_pinball.bin",
			Warlords => "warlords.bin",
			WizardOfWor => "wizard_of_wor.bin",
			WordZapper => "word_zapper.bin",
			YarsRevenge => "yars_revenge.bin",
			Zaxxon => "zaxxon.bin",
		}
	}

	/// Returns the raw binary data of the ROM.
	pub fn data(&self) -> &'static [u8] {
		use BundledRom::*;
		match self {
			Adventure => include_bytes!("../roms/adventure.bin"),
			AirRaid => include_bytes!("../roms/air_raid.bin"),
			Alien => include_bytes!("../roms/alien.bin"),
			Amidar => include_bytes!("../roms/amidar.bin"),
			Assault => include_bytes!("../roms/assault.bin"),
			Asterix => include_bytes!("../roms/asterix.bin"),
			Asteroids => include_bytes!("../roms/asteroids.bin"),
			Atlantis => include_bytes!("../roms/atlantis.bin"),
			Atlantis2 => include_bytes!("../roms/atlantis2.bin"),
			Backgammon => include_bytes!("../roms/backgammon.bin"),
			BankHeist => include_bytes!("../roms/bank_heist.bin"),
			BasicMath => include_bytes!("../roms/basic_math.bin"),
			BattleZone => include_bytes!("../roms/battle_zone.bin"),
			BeamRider => include_bytes!("../roms/beam_rider.bin"),
			Berzerk => include_bytes!("../roms/berzerk.bin"),
			Blackjack => include_bytes!("../roms/blackjack.bin"),
			Bowling => include_bytes!("../roms/bowling.bin"),
			Boxing => include_bytes!("../roms/boxing.bin"),
			Breakout => include_bytes!("../roms/breakout.bin"),
			Carnival => include_bytes!("../roms/carnival.bin"),
			Casino => include_bytes!("../roms/casino.bin"),
			Centipede => include_bytes!("../roms/centipede.bin"),
			ChopperCommand => include_bytes!("../roms/chopper_command.bin"),
			Combat => include_bytes!("../roms/combat.bin"),
			CrazyClimber => include_bytes!("../roms/crazy_climber.bin"),
			Crossbow => include_bytes!("../roms/crossbow.bin"),
			Darkchambers => include_bytes!("../roms/darkchambers.bin"),
			Defender => include_bytes!("../roms/defender.bin"),
			DemonAttack => include_bytes!("../roms/demon_attack.bin"),
			DonkeyKong => include_bytes!("../roms/donkey_kong.bin"),
			DoubleDunk => include_bytes!("../roms/double_dunk.bin"),
			Earthworld => include_bytes!("../roms/earthworld.bin"),
			ElevatorAction => include_bytes!("../roms/elevator_action.bin"),
			Enduro => include_bytes!("../roms/enduro.bin"),
			Entombed => include_bytes!("../roms/entombed.bin"),
			Et => include_bytes!("../roms/et.bin"),
			FishingDerby => include_bytes!("../roms/fishing_derby.bin"),
			FlagCapture => include_bytes!("../roms/flag_capture.bin"),
			Freeway => include_bytes!("../roms/freeway.bin"),
			Frogger => include_bytes!("../roms/frogger.bin"),
			Frostbite => include_bytes!("../roms/frostbite.bin"),
			Galaxian => include_bytes!("../roms/galaxian.bin"),
			Gopher => include_bytes!("../roms/gopher.bin"),
			Gravitar => include_bytes!("../roms/gravitar.bin"),
			Hangman => include_bytes!("../roms/hangman.bin"),
			HauntedHouse => include_bytes!("../roms/haunted_house.bin"),
			Hero => include_bytes!("../roms/hero.bin"),
			HumanCannonball => include_bytes!("../roms/human_cannonball.bin"),
			IceHockey => include_bytes!("../roms/ice_hockey.bin"),
			Jamesbond => include_bytes!("../roms/jamesbond.bin"),
			JourneyEscape => include_bytes!("../roms/journey_escape.bin"),
			Joust => include_bytes!("../roms/joust.bin"),
			Kaboom => include_bytes!("../roms/kaboom.bin"),
			Kangaroo => include_bytes!("../roms/kangaroo.bin"),
			KeystoneKapers => include_bytes!("../roms/keystone_kapers.bin"),
			KingKong => include_bytes!("../roms/king_kong.bin"),
			Klax => include_bytes!("../roms/klax.bin"),
			Koolaid => include_bytes!("../roms/koolaid.bin"),
			Krull => include_bytes!("../roms/krull.bin"),
			KungFuMaster => include_bytes!("../roms/kung_fu_master.bin"),
			LaserGates => include_bytes!("../roms/laser_gates.bin"),
			LostLuggage => include_bytes!("../roms/lost_luggage.bin"),
			MarioBros => include_bytes!("../roms/mario_bros.bin"),
			MazeCraze => include_bytes!("../roms/maze_craze.bin"),
			MiniatureGolf => include_bytes!("../roms/miniature_golf.bin"),
			MontezumaRevenge => include_bytes!("../roms/montezuma_revenge.bin"),
			MrDo => include_bytes!("../roms/mr_do.bin"),
			MsPacman => include_bytes!("../roms/ms_pacman.bin"),
			NameThisGame => include_bytes!("../roms/name_this_game.bin"),
			Othello => include_bytes!("../roms/othello.bin"),
			Pacman => include_bytes!("../roms/pacman.bin"),
			Phoenix => include_bytes!("../roms/phoenix.bin"),
			Pitfall => include_bytes!("../roms/pitfall.bin"),
			Pitfall2 => include_bytes!("../roms/pitfall2.bin"),
			Pong => include_bytes!("../roms/pong.bin"),
			Pooyan => include_bytes!("../roms/pooyan.bin"),
			PrivateEye => include_bytes!("../roms/private_eye.bin"),
			Qbert => include_bytes!("../roms/qbert.bin"),
			Riverraid => include_bytes!("../roms/riverraid.bin"),
			RoadRunner => include_bytes!("../roms/road_runner.bin"),
			Robotank => include_bytes!("../roms/robotank.bin"),
			Seaquest => include_bytes!("../roms/seaquest.bin"),
			SirLancelot => include_bytes!("../roms/sir_lancelot.bin"),
			Skiing => include_bytes!("../roms/skiing.bin"),
			Solaris => include_bytes!("../roms/solaris.bin"),
			SpaceInvaders => include_bytes!("../roms/space_invaders.bin"),
			SpaceWar => include_bytes!("../roms/space_war.bin"),
			StarGunner => include_bytes!("../roms/star_gunner.bin"),
			Superman => include_bytes!("../roms/superman.bin"),
			Surround => include_bytes!("../roms/surround.bin"),
			Tennis => include_bytes!("../roms/tennis.bin"),
			Tetris => include_bytes!("../roms/tetris.bin"),
			TicTacToe3d => include_bytes!("../roms/tic_tac_toe_3d.bin"),
			TimePilot => include_bytes!("../roms/time_pilot.bin"),
			Trondead => include_bytes!("../roms/trondead.bin"),
			Turmoil => include_bytes!("../roms/turmoil.bin"),
			Tutankham => include_bytes!("../roms/tutankham.bin"),
			UpNDown => include_bytes!("../roms/up_n_down.bin"),
			Venture => include_bytes!("../roms/venture.bin"),
			VideoCheckers => include_bytes!("../roms/video_checkers.bin"),
			VideoChess => include_bytes!("../roms/video_chess.bin"),
			VideoCube => include_bytes!("../roms/video_cube.bin"),
			VideoPinball => include_bytes!("../roms/video_pinball.bin"),
			Warlords => include_bytes!("../roms/warlords.bin"),
			WizardOfWor => include_bytes!("../roms/wizard_of_wor.bin"),
			WordZapper => include_bytes!("../roms/word_zapper.bin"),
			YarsRevenge => include_bytes!("../roms/yars_revenge.bin"),
			Zaxxon => include_bytes!("../roms/zaxxon.bin"),
		}
	}
}
