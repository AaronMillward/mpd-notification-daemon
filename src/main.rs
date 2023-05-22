use std::path::PathBuf;

#[cfg(not(feature = "use-mpd-git"))]
use mpd_stable as mpd;

#[cfg(feature = "use-mpd-git")]
use mpd_git as mpd;

use mpd::Idle;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
enum CoverArtMethod {
	/// Don't display any cover art
	None,
	/// Displays cover art without needing the `albumart` mpd function.
	/* The album art hack lets us display the album art without using the "albumart" command.
	but is limited in that it doesn't support anything but a local basic mpd configuration,
	it doesn't respect mounts and the client must be on the same filesystem as the host. */
	LocalHack,
	/// Displays cover art using mpd's `albumart` function. (Requires the `albumart` feature flag.)
	Native,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Config {
	cover_art_method: CoverArtMethod,
	/// Only applies when `cover_art_method` is `LocalHack`.
	/// Should match the music directory in `~/.config/mpd/mpd.conf` or `None` to automatically
	/// detect from `~/.config/mpd/mpd.conf`
	music_directory: Option<String>,
	/// Timeout in milliseconds, leave empty for system default.
	notification_timeout: Option<u32>,
	/// Max connection retries, 0 for unlimited.
	max_connection_retries: u32,
	format: String,
}

impl Default for Config {
	fn default() -> Self {
		Self {
			cover_art_method: CoverArtMethod::LocalHack,
			music_directory: None,
			notification_timeout: None,
			max_connection_retries: 5,
			format: "%Artist - %Title".into(),
		}
	}
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let mut config: Config = confy::load("mpdnd", None)?;

	if !cfg!(feature = "albumart") && matches!(config.cover_art_method, CoverArtMethod::Native) {
		eprintln!("trying to use native album art method without albumart feature! falling back to hack.");
		config.cover_art_method = CoverArtMethod::LocalHack
	}

	let mut client = connect_client(config.max_connection_retries)?;

	/* TODO: You can get the music_directory from the client if it is connected by local socket 
	Although I'm unsure if we should be using listmounts but it just seems to error when used here.
	*/
	let music_directory = if matches!(config.cover_art_method, CoverArtMethod::LocalHack) {
		config.music_directory.clone().map(PathBuf::from)
			.or_else(|| {
				/* Auto read the mpd config for the music directory. */
				/* XXX: Is all this too hacky? */
				use std::io::BufRead;
				let f = std::fs::File::open(
					dirs::home_dir()?.join(".config/mpd/mpd.conf")
				).ok()?;
				let mut opt = None;
				for line in std::io::BufReader::new(f).lines().flatten() {
					if line.starts_with("music_directory") {
						let mut dir = line.split_whitespace().nth(1)?.replace('"', "");
						if dir.starts_with('~') {
							dir = dir.replacen('~', dirs::home_dir()?.to_str()?, 1);
						}
						opt = Some(std::path::PathBuf::from(dir));
						break;
					}
				}
				opt
			})
	} else { None };

	let mut previous_notification_id = None;
	let mut previous_song_id = None;

	loop {
		match notification_loop(&config, &mut client, &mut previous_song_id, &mut previous_notification_id, &music_directory) {
			Ok(_) => {},
			Err(e) => {
				match e {
					mpd::error::Error::Io(e) => {
						match e.kind() {
							/* These are the only errors I've observed coming from killing the MPD server. */
							std::io::ErrorKind::BrokenPipe |
							std::io::ErrorKind::ConnectionReset => {
								show_notification(
									notify_rust::Notification::new()
										.hint(notify_rust::Hint::Category("MPD".into()))
										.summary("MPD Disconnected")
										.icon("network-wired-disconnected"),
								&mut previous_notification_id);

								client = connect_client(config.max_connection_retries)?;
								
								show_notification(
									notify_rust::Notification::new()
										.hint(notify_rust::Hint::Category("MPD".into()))
										.summary("MPD Reconnected")
										.icon("network-wired"),
								&mut previous_notification_id);
							},
							_ => panic!("unexpected IO error in main loop: {}", e),
						}
					},
					e => {
						eprintln!("encountered error in main loop, disgarding and continuing loop: {}", e)
					},
				}
			},
		}
	}
}

fn notification_loop(
	config: &Config,
	client: &mut mpd::Client,
	previous_song_id: &mut Option<mpd::Id>,
	previous_notification_id: &mut Option<u32>,
	music_directory: &Option<PathBuf>
) -> mpd::error::Result<()> {
	client.wait(&[mpd::idle::Subsystem::Player])?;

	let mut notification = notify_rust::Notification::new();

	notification
		.hint(notify_rust::Hint::Category("MPD".into()));
	
	if let Some(t) = config.notification_timeout {
		notification
			.timeout(std::time::Duration::from_millis(t.into()));
	}

	let status = client.status()?;
	
	match status.state {
		mpd::State::Stop => {
			notification
				.summary("MPD Stopped")
				.icon("media-playback-stop");
		},
		mpd::State::Play => {
			/* Determine if song was replayed or is new. */
			{
				let current_song_id = status.song.map(|s| s.id);
	
				let is_the_same_song = *previous_song_id == current_song_id;
	
				if is_the_same_song {
					/* XXX: The user could also spam this by scrubbing back and forth. */
					/* TODO: Maybe add a timeout?  */
					let has_just_started = status.elapsed.expect("should have elapsed time when playing.").is_zero();
					if has_just_started {
						notification
							.icon("media-skip-backward")
							.summary("Playing Again");
					} else {
						/* This is the resume playback case, so we don't need a notification. */
						return Ok(());
					}
				} else {
					notification
						.icon("media-playback-start")
						.summary("Now Playing");
				}
	
				*previous_song_id = current_song_id;
			}
			fill_notification_with_current_song_info(config, client, &mut notification, music_directory)?;
		},
		mpd::State::Pause => return Ok(()),
	}
	
	show_notification(&mut notification, previous_notification_id);
	Ok(())
}

fn fill_notification_with_current_song_info(
	config: &Config,
	client: &mut mpd::Client,
	notification: &mut notify_rust::Notification,
	music_directory: &Option<PathBuf>
) -> mpd::error::Result<()> {
	let song = &client.currentsong()?.expect("should have a song when player is playing.");

	match config.cover_art_method {
		CoverArtMethod::None => {},
		CoverArtMethod::LocalHack => {
			if let Some(dir) = music_directory {
				/* Check if the album art is alongside the song file. 
				if not check one directory up in case the file is nested in for example a "CD1" directory */
				let mut song_path = dir.join(&song.file).with_file_name("cover.jpg");
				if !song_path.exists() {
					song_path = song_path.parent().expect("file should be in a directory.").parent().expect("file shouldn't be directly under root.").join("cover.jpg");
				}
				if song_path.exists() {
					notification.icon = song_path.to_string_lossy().to_string();
				}
			}
		},
		CoverArtMethod::Native => {
			#[cfg(feature = "albumart")]
			/* The icon isn't essential to the notification so we can continue without it if there are errors. */
			/* We already have all the info we need to make the notif at this point in
			the function so a connection error doesn't even need to be returned here. */
			match client.albumart(song) {
				Ok(data) => {
					let path = std::env::temp_dir().join("mpdnd-cover");
					match std::fs::File::create(&path) {
						Ok(mut f) => {
							use std::io::Write;
							f.write_all(&data).unwrap();
							notification.icon(&path.to_string_lossy());
						},
						Err(e) => eprintln!("failed to open album art temp file due to IO error: {}", e),
					}
				},
				Err(e) => {
					/* XXX: I haven't dug deep into this but it seems like `BadPair` here just means "Couldn't find album art"
					bv=ut we still want to report other errors. */
					if !matches!(e, mpd::error::Error::Parse(mpd::error::ParseError::BadPair)) {
						eprintln!("failed to retrieve album art due to mpd error: {}", e);
					}
				}
				
			}
		}
	}
	
	let artist = {
		let album_artist = song.tags.iter().find(|s| s.0 == "AlbumArtist").map(|(_,v)| v);
		#[cfg(feature = "use-mpd-git")]
		let artist = song.artist.as_ref();
		#[cfg(not(feature = "use-mpd-git"))]
		let artist = song.tags.iter().find(|s| s.0 == "Artist").map(|(_,v)| v);
		
		match (artist, album_artist) {
			(None, None) => "<UNKNOWN ARTIST>",
			(None, Some(s)) => s,
			(Some(s), None) => s,
			(Some(s1), Some(s2)) => {
				if s2 == "Various Artists" {
					s1
				} else {
					s2
				}
			},
		}
	};

	notification
		.body({
			&config.format
				.replace(r"\n", "\n")
				.replace("%Artist", artist)
				/* This awful line works on both versions of the mpd crate where
				in stable `tags` is a BTree<String, String> and in git is a Vec<String, String> */
				.replace("%Album", song.tags.iter().find(|s| s.0 == "Album").map(|(_,v)| v).map(String::as_str).unwrap_or("<UNKNOWN ALBUM>"))
				.replace("%Title", song.title.as_deref().unwrap_or("<UNKNOWN TITLE>"))
				.replace("%Date", song.tags.iter().find(|s| s.0 == "Date").map(|(_,v)| v).map(String::as_str).unwrap_or("<UNKNOWN DATE>"))
		});
	
	Ok(())
}

fn show_notification(notification: &mut notify_rust::Notification, previous_notification_id: &mut Option<u32>) {
	/* XXX: I don't know how ids work, is it possible for them to be reused by another program? */

	if let Some(id) = previous_notification_id {
		notification.id(*id);
	}

	let res = notification.show();
		
	match res {
		Ok(s) => {
			*previous_notification_id = Some(s.id());
		},
		Err(e) => {
			eprintln!("Failed to show mpd notification: {}", e);
		},
	}
}

fn connect_client(max_tries: u32) -> mpd::error::Result<mpd::Client> {
	/* Get MPD host from environment */
	let host = std::env::var("MPD_HOST").unwrap_or("127.0.0.1".into());
	let port = std::env::var("MPD_PORT").unwrap_or("6600".into());
	let address = host + ":" + &port;

	let mut tries = 0;
	loop {
		let res = mpd::Client::connect(&address);
		match &res {
			Ok(_) => {
				eprintln!("Successfully connected client on {}", address);
				break res
			},
			Err(e) => {
				if max_tries == 0 {
					eprintln!("Failed to connect client on {} due to error {}, try ({}) retrying in 5 seconds...", address, e, tries + 1);
				} else {
					eprintln!("Failed to connect client on {} due to error {}, try ({}/{}) retrying in 5 seconds...", address, e, tries + 1, max_tries);
					tries += 1;
					if tries == max_tries {
						eprintln!("Failed to connect client after {} retries", tries);
						break res;
					}
				}
				std::thread::sleep(std::time::Duration::from_secs(5));
				continue;
			},
		}
	}
}