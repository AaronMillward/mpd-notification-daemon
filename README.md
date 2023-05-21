# mpd-notification-daemon
A small program to send desktop notifications when MPD changes track.

## Why Does This Exist?
Some desktop environments (in my case awesome and hyprland) don't send notifications related to MPD. A number of MPD clients have scripting that can be used to do this however this requires leaving the client open so I created this program to do it in the background regardless of environment.

## Current Limitations
### Album Art
The MPD library used in this project doesn't support the `albumart` command (in its stable version we could set it as a feature flag) so a workaround is in place to handle album art icons. 

The program will need to know where the music directory is, this is the directory given in `mpd.conf` it will attempt to read this automatically or it can be provided in the `MPD_MUSIC_DIRECTORY` environment variable.

You will likely run into problems running any uncommon MPD setups such as a remote server or having more than a single music directory mounted in MPD.

## TODO

### General
- Native MPD `albumart` method
- Connect/disconnect notification
### Customization
- Custom song display format instead of default `Artist - Title`
- Enable/Disable icons
- Configurable notification timeout