# mpd-notification-daemon
A small program to send desktop notifications when MPD changes track.

## Why Does This Exist?
Some desktop environments (in my case awesome and hyprland) don't send notifications related to MPD. A number of MPD clients have scripting that can be used to do this however this requires leaving the client open so I created this program to do it in the background regardless of environment.

## Configuration

|Name|Description|
|---|---|
|`cover_art_method`|How the cover art for the icon retrieved can be `'None'`, `'LocalHack'`, `'Native'` see below for a description of each.|
|`music_directory`|Only applies when `cover_art_method` is `'LocalHack'`, the directory should match the music directory in `~/.config/mpd/mpd.conf` or leave missing to automatically detect from `~/.config/mpd/mpd.conf`|
|`notification_timeout`|Timeout in milliseconds, leave missing for system default.|
|`max_connection_retries`|Max connection retries, 0 for unlimited.|
|`format`|The format of the notification body text. currently supports `%Artist`, `%Album`, `%Title`, `%Date` and `\n` new lines. `%Artist` is gathered from the MPD `AlbumArtist` tag or if missing falls back to the `Artist` tag. The `Artist` tag will also be used if `AlbumArtist` is "Various Artists" |

### Cover Art Methods
- `'None'` - Don't show any cover art.
- `'LocalHack'` - Simply grabs the cover art path from the local filesystem to reduce IO, you will likely run into problems running MPD setups such as a remote server or having more than a single music directory mounted.
- `'Native'` - Uses an MPD command to get the image, this transfers the whole high resolution image across the connection. this requires the `albumart` feature flag as it uses unstable features.