// Data: sizes, export, combat-log compression and the watcher switch, then the lines other addons wrote.

import { el } from "../lib.js";
import * as compress from "./compress.js";
import * as addons from "./addons.js";

export async function render(body) {
  body.innerHTML = "";
  const stored = el("div");
  const other = el("div");
  body.append(stored, el("h3", "section-title", "Lines from other addons"), other);
  await Promise.all([compress.render(stored), addons.render(other)]);
}
