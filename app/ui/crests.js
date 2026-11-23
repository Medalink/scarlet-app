// The crest drawer of design/crests/crests.js (CRESTS.md), ported as it is: `data-*` attributes in, one SVG out.

import { GLYPHS } from "./glyphs.js";
import { INK, LOOKS } from "./tiers.js";

const BOX = 96;
const CX = 48;
const CY = 50;
const f = (n) => +n.toFixed(2);
const poly = (pts) => "M" + pts.map((p) => f(p[0]) + " " + f(p[1])).join("L") + "Z";
const flip = (pts) => pts.map(([x, y]) => [BOX - x, y]).reverse();
const both = (shapes) => shapes.map(poly).join("") + shapes.map((s) => poly(flip(s))).join("");
const rad = (deg) => (deg * Math.PI) / 180;

const blade = (rx, ry, deg, len, width, belly = 0.5) => {
  const dx = -Math.cos(rad(deg)), dy = -Math.sin(rad(deg)), nx = -dy, ny = dx, h = width / 2;
  return [
    [rx + nx * h * 0.7, ry + ny * h * 0.7],
    [rx + dx * len * belly + nx * h, ry + dy * len * belly + ny * h],
    [rx + dx * len, ry + dy * len],
    [rx + dx * len * belly - nx * h, ry + dy * len * belly - ny * h],
    [rx - nx * h * 0.7, ry - ny * h * 0.7],
  ];
};

const WINGS = [
  blade(30, 36, 52, 30, 9, 0.66),
  blade(29, 42, 37, 30, 9, 0.66),
  blade(29, 48, 22, 27.5, 9, 0.66),
  blade(30, 54, 7, 24, 8.5, 0.66),
  blade(32, 60, -9, 19, 8, 0.66),
];
const LAURELS = [26, 48, 70, 92, 114, 136].map((a, i) => {
  const r = 33.5, x = CX - Math.sin(rad(a)) * r, y = CY + Math.cos(rad(a)) * r;
  return blade(x + Math.cos(rad(a)) * 3, y + Math.sin(rad(a)) * 3, a - 90 + 34, 14.5 - i * 0.4, 7.6, 0.45);
});
const SPURS = [blade(36, 50, 0, 30, 15, 0.36), blade(36, 42, 34, 28, 12, 0.4), blade(36, 58, -34, 28, 12, 0.4)];
const RAYS = [-54, -36, -18, 0, 18, 36, 54].map((a, i) => {
  const len = [40, 45, 42, 45, 42, 45, 40][i], w = 6.5;
  const dx = Math.sin(rad(a)), dy = -Math.cos(rad(a)), nx = -dy, ny = dx;
  return [
    [CX + dx * 16 + nx * w * 0.5, CY + dy * 16 + ny * w * 0.5],
    [CX + dx * len, CY + dy * len],
    [CX + dx * 16 - nx * w * 0.5, CY + dy * 16 - ny * w * 0.5],
  ];
});

export const PIECES = {
  "shield-character": { d: "M24 25H72V47C72 61 62 70 48 77C34 70 24 61 24 47Z", box: [24, 25, 72, 77], fill: "up" },
  "shield-class": { d: "M33 25H63L72 34V66L63 75H33L24 66V34Z", box: [24, 25, 72, 75], fill: "up" },
  "shield-account": { d: "M48 25a25 25 0 1 0 0 50a25 25 0 1 0 0-50Z", box: [23, 25, 73, 75], fill: "up" },
  "ring-account": { d: "M48 20.2a29.8 29.8 0 1 0 0 59.6a29.8 29.8 0 1 0 0-59.6ZM48 22.4a27.6 27.6 0 1 1 0 55.2a27.6 27.6 0 1 1 0-55.2Z", box: [18.2, 20.2, 77.8, 79.8], fill: "up" },
  crown: { d: "M34.5 29L32 14.5L41 21L48 11L55 21L64 14.5L61.5 29Z", box: [32, 11, 64, 29], fill: "right", back: true },
  "great-crown": { d: "M32.5 29L28.5 15.5L35 21.5L38.5 10.5L43.5 20L48 8.5L52.5 20L57.5 10.5L61 21.5L67.5 15.5L63.5 29Z", box: [28.5, 8.5, 67.5, 29], fill: "right", back: true },
  gem: { d: "M48 20.4L51.6 24.6L48 28.8L44.4 24.6Z", box: [44.4, 20.4, 51.6, 28.8], fill: "up" },
  banner: { d: "M15 66H81L76.5 71.5L81 77H15L19.5 71.5Z", box: [15, 66, 81, 77], fill: "right", back: true },
  wings: { d: both(WINGS), box: [7, 10, 89, 66], fill: "out" },
  laurels: { d: both(LAURELS), box: [6, 16, 90, 88], fill: "up" },
  spurs: { d: both(SPURS), box: [6, 24, 90, 76], fill: "out" },
  rays: { d: RAYS.map(poly).join(""), box: [14, 5, 82, 28], fill: "up", back: true },
};

export const SCOPES = {
  character: { shield: "shield-character", side: "wings", glyphY: 45.5 },
  class: { shield: "shield-class", side: "spurs", glyphY: 47 },
  account: { shield: "shield-account", side: "laurels", ring: "ring-account", glyphY: 47 },
};
const ATTACHES = [null, "shield", "crown", "banner", "side", "rays"];

const GHOST = 0.14, GHOST_NEAR = 0.34, NEAR_AT = 0.9, UNEARNED_FILL = 0.34;
const FULL_BOX = "6 5 84 84", COMPACT_BOX = "13 8.5 70 70";
const GLYPH = 28;

const NS = "http://www.w3.org/2000/svg";
const S = (tag, attrs = {}, kids = []) => {
  const e = document.createElementNS(NS, tag);
  for (const k in attrs) if (attrs[k] != null) e.setAttribute(k, attrs[k]);
  for (const c of [].concat(kids)) if (c) e.append(c);
  return e;
};
let uid = 0;

/**
 * Draws the crest of `el` from its data attributes: `crest` (a glyph name), `scope` (character, class or account),
 * `tier` (0 to 5), `progress` (0 to 1 toward the next tier), `size` (px), `state` ("earned" plays the attach, "next"
 * draws the next tier with an empty field), `icon` (a spell icon address) and `label`.
 */
export function drawCrest(el) {
  const held = +el.dataset.tier || 0, size = +el.dataset.size || 64, scope = SCOPES[el.dataset.scope] ? el.dataset.scope : "character";
  const p = Math.max(0, Math.min(1, +el.dataset.progress || 0)), state = el.dataset.state || "", name = el.dataset.crest || "";
  const compact = size < 44, tiny = size < 22, sc = SCOPES[scope];
  const outline = state === "next", tier = outline ? Math.min(5, held + 1) : held;
  const look = LOOKS[tier], metal = look.lead, next = LOOKS[tier + 1]?.lead;
  const id = "cr" + ++uid;
  const svg = S("svg", { viewBox: compact ? COMPACT_BOX : FULL_BOX, width: size, height: size, role: "img", "aria-label": el.dataset.label || "" });
  const defs = S("defs");
  svg.append(defs);
  const about = (k) => `translate(${CX} ${CY}) scale(${k}) translate(${-CX} ${-CY})`;
  const group = (slot, kids) => S("g", { class: `cr-p cr-${slot}${state === "earned" && ATTACHES[tier] === slot ? " cr-new" : ""}` }, kids);
  const path = (key, attrs) => S("path", { d: PIECES[key].d, ...attrs });
  const backed = (key, attrs) => [path(key, { fill: INK, stroke: INK, "stroke-width": compact ? 4.4 : 3.6, "stroke-linejoin": "round" }), path(key, attrs)];
  const flash = (key) => (state === "earned" ? path(key, { fill: "#fff", class: "cr-flash", opacity: 0 }) : null);

  const ghost = (key, color, back, strength = 1) => {
    const pc = PIECES[key], [x0, y0, x1, y1] = pc.box;
    const rect = pc.fill === "up" ? { x: x0, y: y1 - (y1 - y0) * p, width: x1 - x0, height: (y1 - y0) * p }
      : pc.fill === "right" ? { x: x0, y: y0, width: (x1 - x0) * p, height: y1 - y0 }
      : { x: CX - 24 - (CX - 24 - x0) * p, y: y0, width: 2 * (24 + (CX - 24 - x0) * p), height: y1 - y0 };
    defs.append(S("clipPath", { id: id + key }, S("rect", rect)));
    const dim = p >= NEAR_AT ? GHOST_NEAR : GHOST;
    return [
      back ? path(key, { fill: INK, stroke: INK, "stroke-width": 3.6, "stroke-linejoin": "round" }) : null,
      path(key, { fill: color, opacity: dim }),
      path(key, { fill: color, opacity: strength, "clip-path": `url(#${id + key})`, class: "cr-fill" }),
    ];
  };

  const want = tier < 5 && !tiny && !outline ? ATTACHES[tier + 1] : null;
  const hidden = compact && (want === "side" || want === "rays");
  if (look.side && !compact) svg.append(group("side", [path(sc.side, { fill: look.side }), flash(sc.side)]));
  if (want === "side" && !hidden) svg.append(group("side ghost", ghost(sc.side, next)));
  if (look.rays && !compact) svg.append(group("rays", [...backed("rays", { fill: look.rays }), flash("rays")]));
  if (want === "rays" && !hidden) svg.append(group("rays ghost", ghost("rays", next, true)));

  const body = [];
  if (tier === 0) {
    if (!tiny) body.push(...ghost(sc.shield, LOOKS[1].lead, false, UNEARNED_FILL));
    body.push(path(sc.shield, { fill: "none", stroke: metal, "stroke-width": compact ? 2.8 : 2 }));
    if (sc.ring) body.push(path(sc.ring, { fill: metal }));
  } else {
    const rim = metal;
    if (!compact) body.push(path(sc.shield, { fill: INK, transform: about(sc.ring ? 1.3 : 1.09) }));
    if (sc.ring) body.push(path(sc.ring, { fill: rim }));
    body.push(path(sc.shield, { fill: rim }));
    body.push(path(sc.shield, { fill: INK, transform: about(compact ? 0.86 : 0.9) }));
    const solid = look.fieldOpacity === 1 || tiny;
    if (!outline) body.push(path(sc.shield, { fill: look.field, "fill-opacity": solid ? 1 : look.fieldOpacity, transform: about(solid ? (compact ? 0.8 : 0.84) : compact ? 0.86 : 0.9) }));
    body.push(flash(sc.shield));
  }
  svg.append(group("shield", body));

  if (!tiny) {
    const ink = outline ? metal : look.glyph;
    const gs = (compact ? 1.12 : 1) * (GLYPH / 24), gy = sc.glyphY + (compact && tier < 3 ? 1.5 : 0);
    const g = S("g", { class: "cr-glyph", transform: `translate(${CX - 12 * gs} ${gy - 12 * gs}) scale(${gs})`, fill: "none", stroke: ink, "stroke-width": look.fieldOpacity === 1 && !outline ? 2.1 : compact ? 2 : 1.8, "stroke-linecap": "round", "stroke-linejoin": "round", opacity: tier ? 1 : 0.75 });
    if (el.dataset.icon) {
      defs.append(S("clipPath", { id: id + "icon" }, S("rect", { x: 2, y: 2, width: 20, height: 20, rx: 2.4 })));
      g.append(S("image", { "data-icon": el.dataset.icon, x: 2, y: 2, width: 20, height: 20, "clip-path": `url(#${id}icon)`, style: "filter:grayscale(1) contrast(1.1)" }));
      g.append(S("rect", { x: 2, y: 2, width: 20, height: 20, rx: 2.4, fill: tier ? look.glyph : metal, stroke: "none", style: "mix-blend-mode:multiply" }));
    } else {
      g.innerHTML = (name.startsWith("m-") ? '<rect x="1.2" y="1.2" width="21.6" height="21.6" rx="2.6" stroke-width="1" opacity="0.5"/>' : "") + (GLYPHS[name] || "");
    }
    svg.append(g);
  }

  if (tier >= 3) svg.append(group("banner", [...backed("banner", { fill: metal }), flash("banner")]));
  if (want === "banner") svg.append(group("banner ghost", ghost("banner", next, true)));
  if (tier >= 2) {
    const key = tier === 5 ? "great-crown" : "crown";
    svg.append(group("crown", [...backed(key, { fill: metal }), look.gem ? path("gem", { fill: look.gem, stroke: INK, "stroke-width": 1.2 }) : null, flash(key)]));
  }
  if (want === "crown") svg.append(group("crown ghost", ghost("crown", next, true)));
  el.classList.add("sc-crest", "t" + held, "s-" + scope);
  el.replaceChildren(svg);
}
