# How it works

What was read from the game files, what was measured on a live broadcast, and what is still assumed.

## From the game files (DayZ 1.30, Nasdara)

- For every active bunker the server builds two words, a 6-digit grid and a 4-digit lock code, and hands them to the
  engine's `BroadcastTextAsMorse`. The encoding happens in native code and there is no script-visible client hook, so a
  client mod cannot read the codes. The audio is the only way to get them.
- Playback follows the server's `cfgbunkerbroadcast.json`: dot 0.5 s, dash 1.5 s, gap between symbols 0.4 s, 6 s pause
  between bunkers, 3 of the 23 bunkers active, a report every 10 game minutes, broadcast window 17:45 to 06:00.
- The beep is a tone at 988 Hz (987.8 to 988.5 Hz in the samples) with harmonics around 1976, 2842 and 3828 Hz, over a
  broadband static bed around 2.2 to 2.8 kHz (measured from the game's `MorseCode_Short` and `MorseCode_Long` samples).
  The game's `MorseCode_Notification` sample is a different sound: 2.7 s of tone bursts at 926 to 947 Hz over the same
  static.

## Measured on a live broadcast (2026-10-23)

- Digits use standard Morse. A decoded grid matched two real bunkers exactly (bunker 16 = 087088, bunker 17 = 133021),
  and a third (bunker 19 = 065090) over several passes.
- A symbol owns a time window (dot 0.5 s, dash 1.5 s) with the beep at the start (about 0.09 s and 0.48 s audible).
  After the window comes a gap: 0.4 s inside a digit and 0.8 s after the last symbol of a digit. Start to start that is
  0.9 s and 1.9 s inside a digit, 1.3 s and 2.3 s at the end of one. Grid and lock code are not separated by a longer
  pause: a message is simply 10 digits. One bunker takes about 90 s and three bunkers follow each other.
- About 6 percent of beeps were lost, and the tone level sometimes drops out in the middle of a beep. That is why the
  decoder does not count beeps.

## Decoding

The decoder is the `nasdara_decoder` crate. It takes mono 16-bit samples at 44.1 kHz and has no dependencies.

1. `tone`: a Goertzel filter bank around 988 Hz, compared with ten reference bins that avoid the harmonics and the
   static bed. The level is the tone's strength over the seven quietest of them in dB, so broadband noise such as wind
   raises both and cancels, and a note or hum sitting in up to three of the bins does not hide the beep. A frame whose
   spectral peak lies just outside the tone bins (outside roughly 945 to 1025 Hz) is capped at 4 dB: the game's
   notification jingle, a siren or a held note are not beeps.
2. `runs`, `bursts`: hysteresis (on at 15 dB, off at 8 dB) groups tone into beeps, and a burst ends after 5 s of silence.
   A burst of fewer than 20 beeps is ignored.
3. `grid`: a Viterbi search over digit boundaries. Each of the 10 digits is scored as a whole against the tone trace on
   the fixed time grid above, and a digit may start up to 0.12 s from where the previous one predicts. A digit whose
   beeps were not heard becomes `?`.
4. `bunkers`: the grid is matched to the 23 bunker positions, with `?` as a wildcard (four or more digits heard and one
   bunker fits) or with one heard digit corrected (five or more heard).
5. `ledger`: the lock code is read digit by digit over every repeat of that bunker, weighted by how clean each digit was.
   A new code, which a new batch of bunkers brings, replaces the old votes when two confident digits disagree.

While a message is still arriving, `Pipeline::preview` runs the same search over the part heard so far. Its digits are
tentative: the digit being sent is fitted to beeps that are still coming (three dashes of a 0 look like a 2 with its dots
lost), so the window shows a digit only once the next one is due, and shows it dimmed until the message is complete.

A lost first dash of a digit looks exactly like a later-starting digit with a missing dot, so a single pass can be wrong.
That is what the vote over repeats fixes.

## Other sounds

Measured by mixing interferers into synthetic transmissions (beep length and pitch varied, 20 percent of beeps with a
dropout, 3 percent lost; 40 records each) and comparing the share of the 10 digits read correctly. Interferers: held
sines, a swept sine, harmonic stacks, white and low-passed noise, the game's `MorseCode_Notification` and
`MorseCode_StaticNoise` samples, and one 10-second capture of a Windows mix without a beep. The first column is the
detector before the reference-bin and pitch changes, the second after.

| mixed in | before | after |
| --- | --- | --- |
| nothing | 98.2 | 98.5 |
| Windows mix, 1x / 3x / 5x | 97.5 / 94.2 / 87.0 | 98.0 / 96.2 / 91.2 |
| notes on reference bins (620 + 1380 Hz) | 0 | 94 to 95 |
| steady 935 Hz or 1100 Hz tone | 0 | 98.5 |
| swept tone 800 to 1200 Hz | 20 | 74 |
| notification jingle, looped over the message | 56 | 59 |
| voice-like stacks, f0 197 / 120 Hz | 94 / 91 | 96.5 / 96 |
| low-passed noise (wind) | 80 | 88 |

What still gets through: a sound whose spectral peak sits at 945 to 1025 Hz for longer than a dot (a siren passing
through, a held 1 kHz note) reads as beeps; anything louder than the beep over the whole message; and a sound that
overlaps the beeps is added to them. Not checked on a live broadcast: the app logs only the tone level, never audio, so
the detector cannot be replayed on a real recording.

## Capture

Windows can hand an app the audio of one process (`AudioClient::new_application_loopback_client` in WASAPI) or
everything an output device plays. The app uses the first for `DayZ_x64.exe` and the second as the fallback. Windows
sends no packets while nothing plays, so a pacer emits samples on the wall clock, with zeros for silence, to keep the
stream's time identical to real time.

On some PCs (virtual or spatial audio software) Windows attributes the game's sound to the audio engine instead of the
game process. The process capture then returns silence, which the window reports after 20 seconds, and the system audio
mode works. In that mode the app only accepts a message whose grid is one of the 23 real bunkers, because everything
else on the PC is in the audio too.

## Tests

`cargo test` generates its own audio (`nasdara_decoder::synth`: the tone with the measured harmonics and timing, static,
wind, pings, dropouts, lost beeps, length and pitch variance), so no game audio is used or shipped. The same module backs the
demo in the app. One test also replays beep lengths and gaps measured on the live broadcast (numbers only, in
`crates/decoder/tests/fixtures`).

Decoding was exact in tests with wind up to about half the loudness of a beep. Louder wind was not tested for exactness.
Wind alone never produced a false transmission in tests.

## Assumed

- The in-game grid equals iZurvive's (`floor(x / 100)` and `floor((16384 - z) / 100)`). Three bunkers matched.
- Other servers use the shipped playback timings. A server that changes them needs a matching `Timing` in
  `crates/decoder/src/grid.rs`.
- Anything beyond one PC and one session (other radios, other audio stacks) is untested.

## The minimap outline

The coast and built-up outlines on the minimap are not a map of the island. `tools/nasdara-outline.ts` counts the
positions of the island's buildings and spawn zones into a coarse grid, blurs it, cuts it at two densities and traces the
edges, so the outline follows where things are built, not the true shoreline. Only the traced loops are kept, in
`crates/app/src/ui/island.rs`. A test checks that all 23 bunkers fall inside the coast outline.
