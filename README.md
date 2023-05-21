# mpd-notification-daemon
A small program to send desktop notifications when MPD changes track.

## Why Does This Exist?
Some desktop environments (in my case awesome and hyprland) don't send notifications related to MPD. A number of MPD clients have scripting that can be used to do this however this requires leaving the client open so I created this program to do it in the background regardless of environment.

## Current Limitations
### Album Art
The MPD library used in this project doesn't support the `albumart` command (in its stable version, we could set it as a feature flag in the future.) so a workaround is in place to handle album art icons.

The program will need to know where the music directory is, this is the directory given in `~/.config/mpd/mpd.conf` it will attempt to read this automatically or it can be provided in the config file.

You will likely run into problems running any uncommon MPD setups such as a remote server or having more than a single music directory mounted in MPD.

## Configuration

|Name|Description|
|---|---|
|`use_cover_art_hack`|The album art hack lets us display the album art without using the "albumart" command. but is limited in that it doesn't support anything but a local basic mpd configuration, it doesn't respect mounts and the client must be on the same filesystem as the host.|
|`music_directory`|Only applies when `use_cover_art_hack` is enabled. the directory should match the music directory in `~/.config/mpd/mpd.conf` or leave blank/missing to automatically detect from `~/.config/mpd/mpd.conf`|
|`notification_timeout`|Timout in milliseconds, leave missing for system default.|
|`max_connection_retries`|Max connection retries, 0 for unlimited.|
|`format`|The format of the notification body text. currently supports `%Artist`, `%Album`, `%Title`, `%Date` and `\n` new lines. `%Artist` is gathered from the MPD `AlbumArtist` tag or if missing falls back to the `Artist` tag. The `Artist` tag will also be used if `AlbumArtist` is "Various Artists" |

## TODO
### General
- Native MPD `albumart` method
### Customization
- Enable/Disable icons