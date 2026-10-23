# Changelog

## 0.1.0

First release.

- Reads the Nasdara bunker broadcast from DayZ's audio and shows the map grid and lock code of each bunker.
- Shows the Morse as it arrives, digit by digit, with tentative digits marked until the message is complete.
- Lock codes are voted digit by digit over every repeat of a bunker's message.
- Opens the bunker's position on iZurvive.
- Falls back to listening to all system audio for PCs where Windows does not hand DayZ's sound to the app.
