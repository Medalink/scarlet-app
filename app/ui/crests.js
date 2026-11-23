// The crest drawer: `data-*` attributes in, one SVG out. Every shape, colour and number it draws comes from crest-kit.js.

import { ATTACHES, BAND, CENTER, GEM, GLYPH_SIZE, INK, MATERIAL, NEAR_AT, PIECES, QUALITY, SCOPES, UNEARNED, VIEW_BOX } from "./crest-kit.js";
import { GLYPHS } from "./glyphs.js";

const [CX, CY] = CENTER;

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
  const q = QUALITY[tier], next = QUALITY[tier + 1], color = q.color;
  const id = "cr" + ++uid;
  const svg = S("svg", { viewBox: compact ? VIEW_BOX.compact : VIEW_BOX.full, width: size, height: size, role: "img", "aria-label": el.dataset.label || "" });
  const defs = S("defs");
  svg.append(defs);
  const about = (k) => `translate(${CX} ${CY}) scale(${k}) translate(${-CX} ${-CY})`;
  const group = (slot, kids) => S("g", { class: `cr-p cr-${slot}${state === "earned" && ATTACHES[tier] === slot ? " cr-new" : ""}` }, kids);
  const path = (key, attrs) => S("path", { d: PIECES[key].d, ...attrs });
  const backed = (key, attrs) => [path(key, { fill: INK, stroke: INK, "stroke-width": compact ? 4.4 : 3.6, "stroke-linejoin": "round" }), path(key, attrs)];
  const paints = {};
  const gradient = (key, top, bottom) => {
    if (!paints[key]) defs.append(S("linearGradient", { id: id + key, x1: 0, y1: 0, x2: 0, y2: 1 }, [S("stop", { offset: 0, "stop-color": top }), S("stop", { offset: 1, "stop-color": bottom })]));
    paints[key] = true;
    return `url(#${id + key})`;
  };
  const made = (material) => gradient(material, MATERIAL[material].top, MATERIAL[material].bottom);
  const flash = (key) => (state === "earned" ? path(key, { fill: "#fff", class: "cr-flash", opacity: 0 }) : null);

  const ghost = (key, color, back, strength = 1, of = next) => {
    const pc = PIECES[key], [x0, y0, x1, y1] = pc.box;
    const rect = pc.fill === "up" ? { x: x0, y: y1 - (y1 - y0) * p, width: x1 - x0, height: (y1 - y0) * p }
      : pc.fill === "right" ? { x: x0, y: y0, width: (x1 - x0) * p, height: y1 - y0 }
      : { x: CX - 24 - (CX - 24 - x0) * p, y: y0, width: 2 * (24 + (CX - 24 - x0) * p), height: y1 - y0 };
    defs.append(S("clipPath", { id: id + key }, S("rect", rect)));
    const dim = p >= NEAR_AT ? of.near : of.ghost;
    return [
      back ? path(key, { fill: INK, stroke: INK, "stroke-width": 3.6, "stroke-linejoin": "round" }) : null,
      path(key, { fill: color, opacity: dim }),
      path(key, { fill: color, opacity: strength, "clip-path": `url(#${id + key})`, class: "cr-fill" }),
    ];
  };

  const want = tier < 5 && !tiny && !outline ? ATTACHES[tier + 1] : null;
  const hidden = compact && (want === "side" || want === "rays");
  if (q.side && !compact) svg.append(group("side", [path(sc.side, { fill: made(q.side) }), flash(sc.side)]));
  if (want === "side" && !hidden) svg.append(group("side ghost", ghost(sc.side, next.color)));
  if (q.rays && !compact) svg.append(group("rays", [...backed("rays", { fill: made(q.rays) }), flash("rays")]));
  if (want === "rays" && !hidden) svg.append(group("rays ghost", ghost("rays", MATERIAL[next.rays].flat, true)));

  const body = [];
  if (tier === 0) {
    if (!tiny) body.push(...ghost(sc.shield, next.color, false, UNEARNED.fill, { ghost: 0, near: next.near }));
    body.push(path(sc.shield, { fill: "none", stroke: color, "stroke-opacity": UNEARNED.rim, "stroke-width": compact ? 2.8 : 2 }));
    if (sc.ring) body.push(path(sc.ring, { fill: color, "fill-opacity": UNEARNED.rim }));
  } else {
    const rim = made(q.frame), solid = q.field === 1 || tiny, inset = compact ? 0.86 : 0.9;
    const inner = solid ? (compact ? 0.8 : 0.84) : compact ? 0.78 : 0.85;
    if (!compact) body.push(path(sc.shield, { fill: INK, transform: about(sc.ring ? 1.3 : 1.09) }));
    if (sc.ring) body.push(path(sc.ring, { fill: rim }));
    body.push(path(sc.shield, { fill: rim }));
    body.push(path(sc.shield, { fill: solid ? INK : color, transform: about(inset) }));
    if (!solid) body.push(path(sc.shield, { fill: INK, transform: about(inner) }));
    if (!outline) body.push(path(sc.shield, { fill: gradient("enamel", color, q.deep), "fill-opacity": tiny ? q.plain : q.field, transform: about(inner) }));
    body.push(flash(sc.shield));
  }
  svg.append(group("shield", body));

  if (!tiny) {
    const ink = outline ? color : q.glyph;
    const gs = (compact ? 1.12 : 1) * (GLYPH_SIZE / 24), gy = sc.glyphY + (compact && tier < 3 ? 1.5 : 0);
    const g = S("g", { class: "cr-glyph", transform: `translate(${CX - 12 * gs} ${gy - 12 * gs}) scale(${gs})`, fill: "none", stroke: ink, "stroke-width": q.field === 1 && !outline ? 2.1 : compact ? 2 : 1.8, "stroke-linecap": "round", "stroke-linejoin": "round", opacity: tier ? 1 : UNEARNED.glyph });
    if (el.dataset.icon) {
      defs.append(S("clipPath", { id: id + "icon" }, S("rect", { x: 2, y: 2, width: 20, height: 20, rx: 2.4 })));
      g.append(S("image", { "data-icon": el.dataset.icon, x: 2, y: 2, width: 20, height: 20, "clip-path": `url(#${id}icon)`, style: "filter:grayscale(1) contrast(1.1)" }));
      g.append(S("rect", { x: 2, y: 2, width: 20, height: 20, rx: 2.4, fill: q.glyph, stroke: "none", style: "mix-blend-mode:multiply" }));
    } else {
      g.innerHTML = (name.startsWith("m-") ? '<rect x="1.2" y="1.2" width="21.6" height="21.6" rx="2.6" stroke-width="1" opacity="0.5"/>' : "") + (GLYPHS[name] || "");
    }
    svg.append(g);
  }

  if (tier >= 3) {
    defs.append(S("clipPath", { id: id + "band" }, S("rect", { x: BAND[0], y: BAND[1], width: BAND[2] - BAND[0], height: BAND[3] - BAND[1] })));
    svg.append(group("banner", [...backed("banner", { fill: made(q.banner) }), path("banner", { fill: color, "clip-path": `url(#${id}band)` }), flash("banner")]));
  }
  if (want === "banner") svg.append(group("banner ghost", ghost("banner", next.color, true)));
  if (tier >= 2) {
    const key = tier === 5 ? "great-crown" : "crown";
    const setting = `translate(${GEM.at[0]} ${GEM.at[1]}) scale(${GEM.setting}) translate(${-GEM.at[0]} ${-GEM.at[1]})`;
    svg.append(group("crown", [...backed(key, { fill: made(q.crown) }), path("gem", { fill: INK, transform: setting }), path("gem", { fill: q.gem }), flash(key)]));
  }
  if (want === "crown") svg.append(group("crown ghost", ghost("crown", next.color, true)));
  el.classList.add("sc-crest", "t" + held, "s-" + scope);
  el.replaceChildren(svg);
}
