# Changelog

## Unreleased

- Quiet games no longer read as nothing. With the game volume turned down, every digit's tone evidence fell below the cut-off and the whole message was dropped, although the decoder had read it right: both live messages checked matched a real bunker's grid exactly. A grid that is exactly one of the 23 bunkers is now kept whatever its evidence, in the live preview too, and the digit bar no longer paints it as a weak reading, and the lock code digits of weak passes now count towards the vote, so a few repeats of a bunker settle its code where one weak pass showed `?`.
- The `--log` file records the decoder's raw reading of every message, also an unreadable one, and the trace file records the audio level per frame, so a bad message can be told apart from a quiet game.

## 0.3.0 (2026-10-25)

- Test mode now plays its beeps through the default output on Windows, in real time (it ran silent at double speed), so the broadcast can be heard as well as seen. If no output can be opened it stays silent.
- Clicking a bunker card (the selected bunker's or the latest result) opens a menu at the pointer with Copy code and Open on web map, as does a right click. The two buttons on the cards are gone, "code copied" shows in the card header instead, and the column gives the room to the bunker tiles, which now show in five columns even in the smallest window.
- Other sounds hide the beep less. A musical note, hum or voice partial sitting on one of the detector's reference frequencies used to raise its noise estimate until the beep vanished; the loudest three of ten reference bins are now left out. A steady tone at another pitch (the game's own notification jingle at 926 to 947 Hz, a siren, a held note near 1 kHz) no longer counts as a beep. On synthetic transmissions with such sounds mixed in, the share of correctly read digits went from 0 percent to 94 to 98 percent for held notes and tones and from 20 to 74 percent for a siren, and it rose by 0.5 to 9 points with the Windows mix and wind.
- The right column no longer scrolls. The latest result (or the card of the bunker you pick) sits at the top, all 23 bunkers are tiles below it in map order (number, lock code and how sure the reading is; click a heard one for its card, and the map marks it), and the Morse cheat sheet appears when the window is tall enough. The parts share the column's height and grow with the window.
- Minimum window size is 900 x 700 (was 760 x 520), so the narrow layout with the bunkers drawer is gone.
- Fewer unclear digits in game: the decoder now accepts a digit that follows its predecessor up to 0.4 s earlier than the nominal gap, which the live broadcast does now and then. Records that showed four or five `?` read in full.

## 0.2.0 (2026-10-23)

A new window layout and a round of finer details.

- The map is the centrepiece: the result strip and the bunkers list sit in a column beside it, and the Morse ribbon, tone trace and the ten digit slots in a bar along the bottom (about a fifth of the window height). Narrow windows keep the list in a drawer and drop the trace.
- Hover a bunker on the map for a card with its lock code, map grid, game coordinates and how often it was heard. Bunkers never heard say so. Pins can be focused with the keyboard and are labelled for screen readers.
- Chamfered HUD frames with corner brackets on the cards, the result strip and the map.
- Digits and labels are centred on the glyph in every box (the fonts' metrics are corrected), and the interface eases in and out instead of switching. The reduce-motion setting still turns all of it off.
- Morse cheat sheet: the ten digits as dots and dashes, a tap plays a pattern, and while a message arrives the digits that fit what was heard so far light up. It can be hidden in the menu.
- Test mode (options menu, or `--demo`) plays synthetic beeps on repeat, so the window can be tried without the game. It is the default off Windows.
- The minimap outlines the island.
- The options in the menu are switches.
- The idle window repaints at 30 frames a second while listening, so the signal strip no longer stutters when a message starts.

## 0.1.0 (2026-10-23)

First release.

- Reads the Nasdara bunker broadcast from DayZ's audio and shows the map grid and lock code of each bunker.
- Shows the Morse as it arrives, digit by digit, with tentative digits marked until the message is complete.
- Lock codes are voted digit by digit over every repeat of a bunker's message.
- Opens the bunker's position on iZurvive.
- Falls back to listening to all system audio for PCs where Windows does not hand DayZ's sound to the app.
- Pin button keeps the window above the game (borderless windowed mode).
- Flashes the taskbar button when a bunker has been read (option, on by default).
- Runs at below-normal process priority so the game keeps the CPU.
- Uses DirectX 12 on Windows and prefers the low-power GPU, leaving the fast one to the game; the idle frame rate is lowered.
- `--gpu-check <file>` lists the graphics adapters, for a window that does not open.
- `THIRD-PARTY-LICENSES.html` with the notices of the bundled libraries is attached to each release.
