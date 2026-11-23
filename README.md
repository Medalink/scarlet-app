# Scarlet app

The desktop app and command line for [Scarlet](https://github.com/Medalink/scarlet), the WoW Forever addon that
records what your character does.

- `companion`: the `scarlet` command. It reads Scarlet's SavedVariables, keeps every recording as compressed
  payloads under `out/<Name-Surname>/`, rebuilds CSV tables, and writes `Digest.lua` and `Items.lua` into its own `ScarletData`
  addon folder, beside `Scarlet`, so the in-game overview can show your whole history.
- `app`: the Windows app (Tauri). It shows a character overview (with the quests, titles and zone maps of the character's whole life and its current instance locks), an activity heatmap, the event log, sessions, the paths
  you travelled on each map, the feats the website counts for a character (with a toast for each new or upgraded one), charts, loot tables, Legacy talents with what each rank measurably did, and how well the recordings compress. When the player quits WoW Forever it comes to
  the front with a summary of the whole run: the time played, the characters, the levels, quests, achievements and deaths, the zones visited and the stats. It can open itself when WoW Forever starts.

Lines Scarlet records from other installed addons come from their own table, `ScarletAddonsDB`, and are kept apart
from the main recording under `out/<Name-Surname>/addons/` (`addons.txt` and `payloads/`). Their codes are in
Scarlet's [docs/ADDONS.md](https://github.com/Medalink/scarlet/blob/main/docs/ADDONS.md).

The line format both read is in Scarlet's [docs/FORMAT.md](https://github.com/Medalink/scarlet/blob/main/docs/FORMAT.md),
and the overview's rows in [docs/OVERVIEW.md](https://github.com/Medalink/scarlet/blob/main/docs/OVERVIEW.md).

## Build

```
cd companion
cargo build --release
target\release\scarlet.exe export --wow "<World of Warcraft folder>" --out <data folder>

cd ..\app
cargo build --release
target\release\scarlet-app.exe
```

On medaviper, `scripts/dev-update.ps1 -Install` keeps `app\target\release\scarlet-app.exe` at the newest main: a
scheduled task rebuilds it in a clone of its own after a ship, once no game has run for 10 minutes, and an open Scarlet
shows "Restart to update".

## Checks before a ship

```
node e2e/run.mjs [--only cli|app] [--no-build] [--exe <scarlet-app.exe>] [--cli <scarlet>] [--keep <dir>]
```

It needs Node 24 and cargo, builds both crates with the dev profile (about 1 s when nothing changed, 9 to 16 s after a
one-file change; printed apart from the checks) and writes an invented World of Warcraft folder into a temp folder.
`ship` runs it after the gates when a change touches `companion/`, `app/` or `e2e/`.
- The cli lane runs the real `scarlet` command on that folder: the export, the addon files, a second export that
  changes nothing, a later session picked up, and an unknown command refused.
- The app lane (Windows only) starts the real app and drives its page: the 15 views for the selected character, the
  run summary of a session's route, the
  Feats view with uploading off (nothing sent to a local stand-in for the website), the
  character filter, the website settings with Counts from other addons on, a saved setting, an export from inside the app, no reported errors and a clean close with nothing written to the
  app's default settings, profile or Documents folders, the Run key or the build folders.
- 36 checks take 3 to 5 s, 2 s of that the app starting.
- The app runs on a Windows desktop of its own (`e2e/hidden-desktop.ps1`), so no window appears and nothing takes the
  keyboard. The command exits 3 if a window of the app is ever found on the desktop in use. Exit 1 is a failed check,
  exit 2 a usage error.
- A failing app check keeps the temp folder and prints the paths of a screenshot and the page text.

## Dependencies

Every locked crate must have been on crates.io for at least 7 days. After `cargo update`, run
`node tools/deps-age.mjs` (it checks both lockfiles and exits 1 on a younger crate, naming the
older version to move it to with `cargo update -p <name>@<version> --precise <older>`).
`--report` also lists locked crates with a newer old-enough version. Git and path dependencies
are not checked; the repo has no npm packages.

## Map art

Settings has a Map art choice. Scarlet dark, the default, shows the Survey look only. OpenMap adds the
Game map look, read from the player's own copy of the OpenMap addon
(https://github.com/openmap-world/openmap) in `<WoW folder>\_forever_\Interface\AddOns\OpenMap`.
Nothing of OpenMap's is copied into this repository or written back to its folder.

The Survey look's base sheets ship with the app: `app/maps-dark/` holds 54 sheets (zones, continents, the
world and the six capital cities), carried as `dark\` beside the exe by the installer, the release build and a
plain `cargo build --release`. A map the pack lacks is drawn from the `maps` folder in the data folder, then
from OpenMap's art.

OpenMap bakes every zone's art into 512 px world tiles (`Data\Forever\ZoneArt.lua` lists them,
`Media\Textures\Forever\MapZones\<i>_<j>.blp` holds them). The app decodes the BLP files (DXT) and cuts
one WebP per map into `<data folder>\openmap\<OpenMap version>-<cut version>`, the first time a map
is opened. Where each map lies on the world comes from the client's own map tables, kept in
`app/src/openmap_maps.json` and made by:

```
node tools/openmap-maps.mjs <tables dir>
```

Cities have no art in the tiles, so they have no Game map look. `scarlet-app.exe --openmap-cut <folder>`
cuts every map into a folder with no window and writes `cut.log` there.

## Map place names

The Map view's Survey look letters each map from `<maps folder>/<uiMapID>.labels.json`, next to the
art and `index.json`, and adds the subzones the player's own lines name. `tools/map-labels.mjs`
writes those files from the client's map and area tables exported as CSV (UiMap, UiMapAssignment,
UiMapXMapArt, WorldMapOverlay, AreaTable, AreaPOI, TaxiNodes):

```
node tools/map-labels.mjs <tables dir> <maps dir>
node tools/map-labels.mjs <tables dir> <maps dir> --ids 1413,1411
```

Cities and battlegrounds have no area rectangles in the client's tables, so their label files are
empty and the map then shows only the names the player's own lines carry. Without a label file the
map is lettered from the player's lines alone.

## License

GPL-3.0. See `LICENSE`.
