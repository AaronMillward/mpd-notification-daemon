use mpd::Idle;

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

	let mut previous_notification_id = None;
	let mut previous_song_id = None;

	loop {
		let updates = client.wait(&[
			mpd::idle::Subsystem::Player,
			]).expect("failed to return from idle.");

		let mut notification = notify_rust::Notification::new();
		notification
			.timeout(2500)
			.hint(notify_rust::Hint::Category("MPD".into()));

		for subsystem in updates {
			match subsystem {
				mpd::Subsystem::Player => {
					player_updated(&mut client, &mut previous_song_id, &mut notification, &mut previous_notification_id)
				},
				_ => unimplemented!("not listening for these subsystems.")
			}
		}
	}
}

fn player_updated(client: &mut mpd::Client, previous_song_id: &mut Option<mpd::Id>, notification: &mut notify_rust::Notification, previous_notification_id: &mut Option<u32>) {
	/* TODO: The MPD crate doesn't have support for the "albumart" command and the `Proto` module isn't public to manually use it. */
	
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

				let is_the_same_song = *previous_song_id == current_song_id;

				if is_the_same_song {
					/* XXX: The user could also spam this by scrubbing back and forth. */
					let has_just_started = status.elapsed.unwrap().is_zero();
					if has_just_started {
						notification
							.icon("media-skip-backward")
							.summary("Playing Again");
					} else {
						/* This is the resume playback case, so we don't need a notification. */
						return;
					}
				} else {
					notification
						.icon("media-playback-start")
						.summary("Now Playing");
				}

				*previous_song_id = current_song_id;
			}
			fill_notification_with_song_info(&client.currentsong().expect("should be able to get current song after wake from idle.").expect("should be Some when player is playing."), notification);
		},
		mpd::State::Pause => return,
	}

	show_notification(notification, previous_notification_id)
}

fn fill_notification_with_song_info(song: &mpd::Song, notification: &mut notify_rust::Notification) {
	let album_artist = song.tags.get("AlbumArtist");
	let artist = song.tags.get("Artist");

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