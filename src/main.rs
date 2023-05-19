use mpd::Idle;

fn main() {
	let mut client = mpd::Client::connect("127.0.0.1:6600").unwrap();

	let mut previous_notification_id = None;
	let mut previous_song_id = None;

	const SHOW_OPTIONS_NOTIFICATIONS: bool = true;

	loop {
		let updates = client.wait(&[
			mpd::idle::Subsystem::Options,
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
				mpd::Subsystem::Options => {
					if SHOW_OPTIONS_NOTIFICATIONS {

					}
				},
				_ => unimplemented!("not listening for these subsystems.")
			}
		}
	}
}

fn player_updated(client: &mut mpd::Client, previous_song_id: &mut Option<mpd::Id>, notification: &mut notify_rust::Notification, previous_notification_id: &mut Option<u32>) {
	let status = client.status().unwrap();

	match status.state {
		mpd::State::Stop => {
			notification
				.summary("MPD Stopped");
		},
		mpd::State::Play => {
			eprintln!("update.");
			/* Determine if song was replayed or is new. */
			{
				let current_song_id = status.song.map(|s| s.id);

				let is_the_same_song = *previous_song_id == current_song_id;

				if is_the_same_song {
					/* XXX: Using 10 Milliseconds as I'm unsure how precise the playback time is. */
					/* XXX: The user could also spam this by scrubbing back and forth. */
					let has_just_started = status.elapsed.unwrap().is_zero();
					if has_just_started {
						notification
							.summary("Playing Again");
					} else {
						/* This is the resume playback case, so we don't need a notification. */
						return;
					}
				} else {
					notification
						.summary("Now Playing");
				}

				*previous_song_id = current_song_id;
			}

			fill_notification_with_song_info(&client.currentsong().unwrap().unwrap(), notification);
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

	let title = song.title.clone().unwrap();

	notification
		.body(format!("{} - {}", display_artist, &title).as_str());
}

fn show_notification(notification: &mut notify_rust::Notification, previous_notification_id: &mut Option<u32>) {
	if let Some(id) = previous_notification_id {
		notification.id(*id);
	}

	let res = notification.show();
		
	match res {
		Ok(s) => {
			*previous_notification_id = Some(s.id());
		},
		Err(e) => {
			*previous_notification_id = None;
			eprintln!("failed to show mpd notification: {}", e);
		},
	}
}