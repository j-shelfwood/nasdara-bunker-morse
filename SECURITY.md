# Security

Bunker Morse runs locally and makes no network connections. Two things are worth knowing when you assess it:

- It opens an iZurvive link in your default browser when you press the map button.
- On Windows it attaches to the audio of the DayZ process (per-process loopback capture) to listen to the game. It does not read DayZ's memory, inject code or touch the game's files.

Report a vulnerability privately through [GitHub security advisories](../../security/advisories/new) for this repository.
Please do not open a public issue for it.
