# active-window

Get metadata of the active (foreground) window:
 
 - window title
 - unique id
 - bounds
 - owner
 - URL of the current browser tab (MacOS only)

### TODO

This crate is not feature-complete yet

- [x] Windows support
- [ ] Linux (X11) support
- [ ] Linux (Wayland) support
- [ ] MacOS support

### Troubleshooting

Users on macOS 10.13 or earlier needs to download the Swift runtime support libraries.

### Acknowledgements

* [active-win: nodejs counterpart and inspiration for this crate](https://github.com/sindresorhus/active-win) 
* [x11_get_windows: implementation for X11](https://github.com/HiruNya/x11_get_windows)