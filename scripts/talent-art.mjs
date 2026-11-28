// Writes the companion's talent art table from the client's exported DB2 csv files:
//   node scripts/talent-art.mjs <csv folder> [--out <file>]
// The folder holds TraitNode, TraitNodeXTraitNodeEntry, TraitNodeEntry, TraitDefinition, TraitEdge, SpellName and
// SpellMisc. Output (default companion/data/talent-art.json, which companion/src/art.rs embeds): `icons` maps every spell a
// trait tree entry names to its icon file id, and `legacy` has the three Legacy trees with each node on its grid cell.
// Only ids are written, never art. Running it twice on one folder changes nothing.
import fs from "node:fs";
import path from "node:path";
import { parseArgs } from "node:util";

const REPO = path.resolve(import.meta.dirname, "..");
const USAGE = "usage: node scripts/talent-art.mjs <csv folder> [--out <file>]";
const { values, positionals } = parseArgs({ options: { out: { type: "string" } }, allowPositionals: true });
if (positionals.length !== 1) {
  console.error(USAGE);
  process.exit(2);
}
const folder = positionals[0];
const out = values.out ?? path.join(REPO, "companion", "data", "talent-art.json");

const LEGACY = [
  [1187, "Professions"],
  [1188, "Adventure"],
  [1189, "Progression"],
];
const LEGACY_STEP = 900;
const PLACEHOLDER_FLAG = 2;

function parseCsv(text) {
  const rows = [];
  let row = [];
  let cell = "";
  let quoted = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quoted) {
      if (c !== '"') cell += c;
      else if (text[i + 1] === '"') (cell += '"'), i++;
      else quoted = false;
    } else if (c === '"') quoted = true;
    else if (c === ",") (row.push(cell), (cell = ""));
    else if (c === "\n" || c === "\r") {
      if (c === "\r" && text[i + 1] === "\n") i++;
      row.push(cell);
      cell = "";
      if (row.length > 1 || row[0] !== "") rows.push(row);
      row = [];
    } else cell += c;
  }
  if (cell !== "" || row.length) rows.push([...row, cell]);
  return rows;
}

function table(name) {
  const file = path.join(folder, `${name}.csv`);
  if (!fs.existsSync(file)) {
    console.error(`${file} is missing\n${USAGE}`);
    process.exit(2);
  }
  const [head, ...rows] = parseCsv(fs.readFileSync(file, "utf8"));
  return rows.map((r) => Object.fromEntries(head.map((h, i) => [h, r[i] ?? ""])));
}

const num = (v) => Number(v) || 0;

const nodes = table("TraitNode");
const nodeEntries = table("TraitNodeXTraitNodeEntry");
const entries = new Map(table("TraitNodeEntry").map((r) => [num(r.ID), r]));
const definitions = new Map(table("TraitDefinition").map((r) => [num(r.ID), r]));
const edges = table("TraitEdge");
const spellNames = new Map(table("SpellName").map((r) => [num(r.ID), r.Name_lang]));

const spellIcons = new Map();
for (const r of table("SpellMisc")) {
  const spell = num(r.SpellID);
  const icon = num(r.SpellIconFileDataID);
  if (icon && !spellIcons.has(spell)) spellIcons.set(spell, icon);
}

const entriesOf = new Map();
for (const r of [...nodeEntries].sort((a, b) => num(a.Index) - num(b.Index))) {
  const list = entriesOf.get(num(r.TraitNodeID)) ?? [];
  list.push(num(r.TraitNodeEntryID));
  entriesOf.set(num(r.TraitNodeID), list);
}

const icons = {};
const bySpell = (spell) => spellIcons.get(spell) ?? 0;
for (const def of definitions.values()) {
  const spell = num(def.SpellID);
  const override = num(def.OverrideIcon);
  if (spell && (override || bySpell(spell))) icons[spell] = override || bySpell(spell);
  for (const named of [num(def.VisibleSpellID), num(def.OverridesSpellID)]) {
    if (named && bySpell(named)) icons[named] ??= bySpell(named);
  }
}

function legacyNode(node) {
  const entry = entries.get(entriesOf.get(num(node.ID))?.[0]);
  const def = definitions.get(num(entry?.TraitDefinitionID));
  const spell = num(def?.SpellID);
  return {
    id: num(node.ID),
    x: num(node.PosX),
    y: num(node.PosY),
    spell,
    name: def?.OverrideName_lang || spellNames.get(spell) || "",
    max: num(entry?.MaxRanks),
    icon: num(def?.OverrideIcon) || bySpell(spell),
    placeholder: (num(node.Flags) & PLACEHOLDER_FLAG) !== 0,
  };
}

const rawLegacy = LEGACY.map(([id, name]) => ({ id, name, nodes: nodes.filter((n) => num(n.TraitTreeID) === id).map(legacyNode) }));
const every = rawLegacy.flatMap((t) => t.nodes);
const minX = Math.min(...every.map((n) => n.x));
const minY = Math.min(...every.map((n) => n.y));
const legacy = rawLegacy.map(({ id, name, nodes: list }) => {
  const ids = new Set(list.map((n) => n.id));
  return {
    id,
    name,
    nodes: list
      .map(({ x, y, ...n }) => ({ ...n, col: Math.round((x - minX) / LEGACY_STEP), row: Math.round((y - minY) / LEGACY_STEP) }))
      .sort((a, b) => a.row - b.row || a.col - b.col),
    edges: edges
      .filter((e) => ids.has(num(e.LeftTraitNodeID)) && ids.has(num(e.RightTraitNodeID)))
      .map((e) => [num(e.LeftTraitNodeID), num(e.RightTraitNodeID)])
      .sort((a, b) => a[0] - b[0] || a[1] - b[1]),
  };
});

const sortedIcons = Object.fromEntries(Object.entries(icons).sort((a, b) => Number(a[0]) - Number(b[0])));
fs.mkdirSync(path.dirname(out), { recursive: true });
fs.writeFileSync(out, `${JSON.stringify({ icons: sortedIcons, legacy })}\n`);
const placeholders = legacy.flatMap((t) => t.nodes).filter((n) => n.placeholder).length;
console.log(`wrote ${out}: ${Object.keys(sortedIcons).length} spell icons, ${legacy.length} Legacy trees with ${every.length} nodes (${placeholders} placeholders)`);
