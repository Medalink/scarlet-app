# Scarlet app

The `scarlet` command (`companion/`) and the Windows desktop app (`app/`, Tauri) for the
[Scarlet addon](https://github.com/Medalink/scarlet). [README.md](README.md) has the build and the checks,
[docs/RELEASING.md](docs/RELEASING.md) the releases. The line format is the addon's
[docs/FORMAT.md](https://github.com/Medalink/scarlet/blob/main/docs/FORMAT.md): change it before changing this repo.

## Rules

- No unit or feature tests. Verify with `cargo fmt --check`, clippy with `-D warnings`, and the real thing:
  `node e2e/run.mjs` drives the real `scarlet` command and, on Windows, the real app.
- `app/` does not compile on Linux (Win32 calls). Its clippy, its build and the app lane of the e2e run only on
  Windows. A ship from medaware checks `app/` formatting and runs the command's lane, printing `skip app`. Before
  shipping a change under `app/`, run clippy there and `node e2e/run.mjs` on Windows.
- Never show a window on the owner's desktop or take its keyboard focus: the owner plays on medaviper while agents
  work. A program with a window under test starts on a Windows desktop of its own through `e2e/hidden-desktop.ps1`
  (CreateDesktop and a job object); browsers run headless. Never offer "a window appears briefly" as a tradeoff:
  build the invisible path first, and prove it with the window handle on the desktop in use being 0 and the
  foreground window unchanged (`e2e/run.mjs` exits 3 when it finds an app window there).
- Run the command and the app only against an invented World of Warcraft folder in a temp or scratch folder, as
  `e2e/run.mjs` does. Never point them at the owner's game folder or data folder.
- A new line code ships in this order: FORMAT.md in Medalink/scarlet first, then its row in scarlet-web's
  `docs/groups.tsv` at once, then the `scarlet-part` revision in `companion/Cargo.toml` here. scarlet-web's
  `part-groups` gate reads FORMAT.md on GitHub main and fails both ways, so the row never ships first. Production can
  lag main, so the upload keeps going when the website answers `unknown_group` (`e68e783`); keep it that way.
- Game art, icons, maps and names come from the game's own files and tables, never from stand-in art or classic-era
  names written from memory: Forever renames some spells and lacks others. `tools/map-labels.mjs` and
  `tools/openmap-maps.mjs` read the client's exported tables. This repo is public: never commit Blizzard's art. The
  Survey sheets in `app/maps-dark/` are drawn by `tools/render-dark.mjs` with our own marks; regenerate them there
  and add no other art.
- Never put a real player name in the repo, a doc, a screenshot or a commit message; examples use `Name-Surname`.
- Never name the owner's local game-file tooling, its folders or its paths in a file, a comment or a commit message. Do
  not name other companies' addons, apps, websites or data services, except the game and its maker (World of Warcraft,
  Blizzard), the addons whose saved data the import reads (`companion/src/import.rs`) or Scarlet records from
  (`docs/ADDONS.md` in the addon repo), OpenMap, and the sites it publishes to (CurseForge, Wago, GitHub).
- Change only the surface the report or design names. A piece several views share changes for all of them: list its
  users, make the change opt-in for the view named, and check one other user in the running app before shipping.
- A backslash in a script passed inline through the Bash tool on medaviper can arrive changed: a doubled backslash
  arrives as one, so an escape meant as text is read by the script's language as a control character (Python reads
  `\0` as a NUL). On 2026-11-22 this put a NUL into `scripts/deploy/ring-entry.sh` in scarlet-web, which shipped and made
  git show the file as binary while `sh -n` still passed. Write any script or patch that holds a backslash with the file
  Write tool, then run it. Scan an edited script for control bytes, and read a diff with `git diff --text` when git says
  Bin.
- The beta UI rebuild is on the branch `feat/beta-ui-parity-rebuild` and stays off main until the owner says in so many
  words to ship the rebuild (held since 2026-11-23). Push that branch only: no `ship`, no merge into main, no deploy of
  it. Approval of how one part looks is not approval to ship. The ship that lands the rebuild deletes this bullet.
- Land every change with `ship`. Open no pull request and run no push to main by hand.
