use std::path::PathBuf;

use mpd::Idle;

/* TODO: The MPD crate doesn't have support for the "albumart" command and the `Proto` module isn't public to manually use it. 
Update: The git version of the crate does support albumart, to use it we would need to call it, save the result to a temp file, then pass that path to the notif.
but should we use the potentially unstable git version to do this? maybe make it a feature flag?
*/
/// The album art hack lets us display the album art without using the "albumart" command. 
///
/// But is limited in that it doesn't support anything but a local basic mpd configuration,
/// it doesn't respect mounts and the client must be on the same filesystem as the host.
/// 
/// To use this hack this variable should be true, you can then either...
/// 1. Set an "MPD_MUSIC_DIRECTORY" environment variable as the music directory
/// 1. Leave the environment variable unset and attempt to automatically read ~/.config/mpd/mpd.conf
const USE_COVER_ART_HACK: bool = true;

fn main() {
	let mut client = {
		/* Get MPD host from environment */
		let host = std::env::var("MPD_HOST").unwrap_or("127.0.0.1".into());
		let port = std::env::var("MPD_PORT").unwrap_or("6600".into());
		let address = host + ":" + &port;

		loop {
			match mpd::Client::connect(&address) {
				Ok(c) => break c,
				Err(e) => {
					eprintln!("Failed to connect to MPD on {} due to error {}, retrying", address, e);
					std::thread::sleep(std::time::Duration::from_secs(5));
					continue;
				},
			}
		}
	};

	/* TODO: You can get the music_directory from the client if it is connected by local socket 
	Although I'm unsure if we should be using listmounts but it just seems to error when used here.
	*/
	let music_directory = if USE_COVER_ART_HACK { 
		std::env::var("MPD_MUSIC_DIRECTORY").ok().map(PathBuf::from)
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
		client.wait(&[mpd::idle::Subsystem::Player]).expect("failed to return from idle.");

		let mut notification = notify_rust::Notification::new();
		notification
			.timeout(2500)
			.hint(notify_rust::Hint::Category("MPD".into()));

		let status = client.status().expect("should be able to get status directly after waking from idle.");
		
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
		
					let is_the_same_song = previous_song_id == current_song_id;
		
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
							continue;
						}
					} else {
						notification
							.icon("media-playback-start")
							.summary("Now Playing");
					}
		
					previous_song_id = current_song_id;
				}
				fill_notification_with_song_info(&client.currentsong().expect("should be able to get current song after wake from idle.").expect("should be Some when player is playing."), &mut notification, &music_directory);
			},
			mpd::State::Pause => continue,
		}
		
		show_notification(&mut notification, &mut previous_notification_id)
	}
}

fn fill_notification_with_song_info(song: &mpd::Song, notification: &mut notify_rust::Notification, music_directory: &Option<PathBuf>) {
	let album_artist = song.tags.iter().find(|s| s.0 == "AlbumArtist").map(|(_,v)| v);
	let artist = song.tags.iter().find(|s| s.0 == "Artist").map(|(_,v)| v);
	
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

	let display_artist = match (artist, album_artist) {
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
	};

	let title = song.title.clone().unwrap_or("<UNKNOWN TITLE>".into());

	notification
		.body(format!("{} - {}", display_artist, &title).as_str());
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