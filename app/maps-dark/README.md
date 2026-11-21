# Dark map sheets

54 WebP sheets, named `<uiMapID>.webp`: 2004 x 1336, and 1024 x 1024 for the two square continent maps
(1463, 1464). The Map view's Survey look draws its base from them, so it works without OpenMap and without
a `maps` folder.

- Drawn by `tools/render-dark.mjs`, which uses the app's own sheet code, `app/ui/views/mapstyle.js`, and
  adds the detail with `tools/survey-detail.js`. Rerun it to remake the sheets; nothing here is edited by
  hand.
- Traced from the game's map shapes: the colour and texture of each map, and the outline of each zone's
  land. The detail is measured from the art and drawn again with the script's own marks: a zigzag crest with
  shade strokes along each mountain range, one line down each road, lines that follow the coast out to sea,
  the edge of the shallows, islets, and in a city each building as a polygon and each wall as straight
  segments. No pixel of the game's art is copied into a sheet.
- 48 sheets are zones, continents and the world, cut from OpenMap's art with each zone's own land mask, so
  neighbouring zones are one flat tone. Their water is read from the unmasked cut (`--sea`), so a hatched
  sea reads as sea. The continents and the world carry the shore only. 6 are cities (1453 Stormwind City,
  1454 Orgrimmar, 1455 Ironforge, 1456 Thunder Bluff, 1457 Darnassus, 1458 Undercity), drawn from the
  owner's extracted maps.
- The installer and the release build carry this folder as `dark\` beside the exe
  (`bundle.resources` in `app/tauri.conf.json`). `map_image_dark` reads it from Tauri's resource folder.
- The maps this pack lacks fall back to the `maps` folder in the data folder, then to OpenMap's art.

To remake them:

```
scarlet-app.exe --openmap-cut <cut>
scarlet-app.exe --openmap-cut <plain> --plain
node tools/render-dark.mjs <cut> app/maps-dark all --sea <plain> --quality 0.7
node tools/render-dark.mjs <maps folder> app/maps-dark 1453,1454,1455,1456,1457,1458 --quality 0.7
```

The game's map art is Blizzard Entertainment's.
