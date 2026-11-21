# Dark map sheets

54 WebP sheets, 2004 x 1336, named `<uiMapID>.webp`. The Map view's Survey look draws its base from them,
so it works without OpenMap and without a `maps` folder.

- Drawn by `tools/render-dark.mjs` (run with `--ink off`), which uses the app's own sheet code,
  `app/ui/views/mapstyle.js`. Rerun it to remake the sheets; nothing here is edited by hand.
- Traced from the game's map shapes: the colour and texture of each map, and the outline of each zone's
  land. No strokes are copied from the game's art.
- 48 sheets are zones, continents and the world, cut from OpenMap's art with each zone's own land mask, so
  neighbouring zones are one flat tone. 6 are cities (1453 Stormwind City, 1454 Orgrimmar, 1455 Ironforge,
  1456 Thunder Bluff, 1457 Darnassus, 1458 Undercity), drawn from the owner's extracted maps.
- The installer and the release build carry this folder as `dark\` beside the exe
  (`bundle.resources` in `app/tauri.conf.json`). `map_image_dark` reads it from Tauri's resource folder.
- The maps this pack lacks fall back to the `maps` folder in the data folder, then to OpenMap's art.

The game's map art is Blizzard Entertainment's.
