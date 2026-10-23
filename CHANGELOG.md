# Changelog

## 0.1.0 (2026-10-23)

First release.

- Reads the Nasdara bunker broadcast from DayZ's audio and shows the map grid and lock code of each bunker.
- Shows the Morse as it arrives, digit by digit, with tentative digits marked until the message is complete.
- Lock codes are voted digit by digit over every repeat of a bunker's message.
- Opens the bunker's position on iZurvive.
- Falls back to listening to all system audio for PCs where Windows does not hand DayZ's sound to the app.
- `--demo` plays synthetic beeps on repeat, so the window can be reviewed without the game. It is the default off Windows.
- Pin button keeps the window above the game (borderless windowed mode).
- Flashes the taskbar button when a bunker has been read (option, on by default).
- Runs at below-normal process priority so the game keeps the CPU.
- Uses DirectX 12 on Windows and prefers the low-power GPU, leaving the fast one to the game; the idle frame rate is lowered.
- `--gpu-check <file>` lists the graphics adapters, for a window that does not open.
- `THIRD-PARTY-LICENSES.html` with the notices of the bundled libraries is attached to each release.
