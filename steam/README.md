# Steam files

Everything here is written by the game (`DUEL_WRITE_STEAM=<folder> duel_arena.exe`, or
`cargo test write_steam_folder -- --ignored`); a test fails when one is out of date.

## Switching to the game's own App ID

1. Put the App ID in `app_id.txt` (the one number on its own line) and rebuild.
2. On the Steamworks partner site:
   - **Stats & Achievements**: enter the stats and achievements from `achievements.vdf` (the API
     names, display names, descriptions, hidden flags and progress stats are all there), with the
     icons from `icons/` (`<id>.png` once earned, `<id>_locked.png` before). Publish.
   - **Steam Cloud**: give the app a byte quota (1 MB is plenty) and a file count of 16; the game
     writes its saves through the Cloud API itself.
   - **Rich Presence**: upload `rich_presence.vdf` as the English localization.
   - **Steam Input**: upload `steam_input_manifest.vdf` as the In-Game Actions file and set the
     default layouts from `controller_xbox.vdf`, `controller_ps.vdf`, `controller_switch.vdf` and
     `controller_steamdeck.vdf` (back grips: dash, kick, quick chat, finisher; right trackpad
     looks like a mouse; gyro aims while guarding).
3. Start the game: what was earned while it ran under 480 goes to Steam on the first start.

## What works under the test App ID 480

Lobbies, relayed play, invites and parties, the friends panel, Join buttons and the plain
presence line work. Achievements, stats and Cloud saves stay on this computer (shown in the game
the same way), friends see the plain status line instead of the localized one, and the game reads
the controller itself. The tests run all of it against a fake Steam (`steam_api::FakeSteam`).
