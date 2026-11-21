# Scarlet app

The desktop app and command line for [Scarlet](https://github.com/Medalink/scarlet), the WoW Forever addon that
records what your character does.

- `companion`: the `scarlet` command. It reads Scarlet's SavedVariables, keeps every recording as compressed
  payloads under `out/<Name-Surname>/`, rebuilds CSV tables, and writes `Digest.lua` into the addon folder so the
  in-game overview can show your whole history.
- `app`: the Windows app (Tauri). It shows a character overview, an activity heatmap, a timeline, sessions, the paths
  you travelled on each map, charts, loot tables and how well the recordings compress. It can open itself when WoW
  Forever starts.

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

## License

GPL-3.0. See `LICENSE`.
