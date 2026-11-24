// Items: what every character carries, where it is as a bar per item, the recipes they know as reagents to product,
// and how fresh each container read is. The model comes from the `items` command.

import { invoke, el, elText, esc, displayName, num, dateTime, moneyHtml, state, tile, tiles, wireTips } from "../lib.js";
import { counted, play } from "../motion.js";

const when = (secs) => (secs ? dateTime(secs * 1000) : "–");

const WHERE = [
  ["Bags", "--s-1", "bags", "in bags"],
  ["Bank", "--s-4", "bank", "in bank"],
  ["Mailbox", "--s-3", "mail", "in mailbox"],
  ["Auctions", "--s-2", "auctions", "in auctions"],
];
const SHARED = ["Shared banks", "--alt-4"];
const READS = [
  ["Bags", "bags_at"],
  ["Bank", "bank_at"],
  ["Mailbox", "mail_at"],
  ["Auctions", "auctions_at"],
];
const FRESH_SECS = 12 * 3600;
const DAY_OLD_SECS = 48 * 3600;
const WHO_SHOWN = 3;

function placeText(view, place, scope, accountUnknown) {
  if (accountUnknown) return `${place} (account unknown, ${displayName(scope)})`;
  const several = view.shared.filter((s) => s.place === place).length > 1;
  return several && scope ? `${place} (${scope})` : place;
}

function key(rows) {
  const k = el("div", "sc-key");
  for (const [label, c, value] of rows) {
    const s = el("span");
    s.style.setProperty("--c", `var(${c})`);
    s.append(label);
    if (value !== undefined) {
      s.append(" ", counted(elText("b", "", num(value)), value, num));
    }
    k.append(s);
  }
  return k;
}

function holdings(row, view) {
  const parts = [];
  for (const h of row.holders) {
    const name = displayName(h.character);
    const mine = WHERE.filter(([, , f]) => h[f] > 0).map(([label, c, f, phrase]) => ({ label, c, n: h[f], phrase }));
    parts.push({ name, n: mine.reduce((a, m) => a + m.n, 0), pieces: mine });
  }
  for (const p of row.places) {
    parts.push({
      name: placeText(view, p.place, p.scope, p.account_unknown),
      n: p.count,
      pieces: [{ label: SHARED[0], c: SHARED[1], n: p.count, phrase: "" }],
    });
  }
  return parts;
}

function itemRow(r, view, maxValue) {
  const parts = holdings(r, view);
  const segs = parts
    .map((h, i) =>
      h.pieces
        .map(
          (m, j) =>
            `<i${i && !j ? ' class="cut"' : ""} style="flex:${m.n};--c:var(${m.c})" data-tip="${esc(h.name)} · ${esc(m.label)}|${num(m.n)}"></i>`
        )
        .join("")
    )
    .join("");
  const who = parts
    .slice(0, WHO_SHOWN)
    .map((h) => {
      const detail = h.pieces[0].phrase ? h.pieces.map((m) => `${num(m.n)} ${m.phrase}`).join(" · ") : `${num(h.n)}`;
      return `<span data-tip="${esc(h.name)}|${esc(detail)}"><b>${esc(h.name)}</b> ${num(h.n)}</span>`;
    })
    .join("");
  const more = parts.length - WHO_SHOWN;
  const value = r.price ? r.price.copper * r.total : 0;
  const price = r.price
    ? `<span data-tip="Last auction price|${esc(when(r.price.at))}">${moneyHtml(r.price.copper)}</span>`
    : "–";
  const worth = r.price
    ? `${moneyHtml(value)}<span class="it-vbar"><i style="--w:${Math.max(2, (value / maxValue) * 100).toFixed(1)}%"></i></span>`
    : "–";
  return `<div class="it-row"><div class="it-name"><b>${esc(r.name || `Item ${r.id}`)}</b> <span class="dim">${r.id}</span></div>
    <div class="it-where"><div class="sc-stack">${segs}</div><div class="it-who">${who}${more > 0 ? `<span class="dim">+${num(more)} more</span>` : ""}</div></div>
    <b class="it-total">${num(r.total)}</b><div class="it-price">${price}</div><div class="it-value">${worth}</div></div>`;
}

function itemList(view, into, sort) {
  const valueOf = (r) => (r.price ? r.price.copper * r.total : -1);
  const rows = view.rows.slice();
  if (sort === "value") rows.sort((a, b) => valueOf(b) - valueOf(a) || b.total - a.total);
  const maxValue = Math.max(...rows.map(valueOf), 1);
  const totals = WHERE.map(([label, c, f]) => [label, c, rows.reduce((a, r) => a + r.holders.reduce((n, h) => n + h[f], 0), 0)]);
  const shared = rows.reduce((a, r) => a + r.places.reduce((n, p) => n + p.count, 0), 0);
  const list = el("div", "it-list");
  list.append(key([...totals.filter((t) => t[2] > 0), ...(shared ? [[SHARED[0], SHARED[1], shared]] : [])]));
  const body = el("div", "it-rows");
  body.innerHTML = `<div class="it-row it-head"><span>Item</span><span>Where</span><span>Total</span><span>Last price</span><span>Stack value</span></div>${rows
    .map((r) => itemRow(r, view, maxValue))
    .join("")}`;
  list.append(body);
  into.appendChild(list);
  if (view.matched > view.rows.length) {
    into.appendChild(el("div", "dim", `Showing ${num(view.rows.length)} of ${num(view.matched)}.`));
  }
}

const itemName = (id, name) => esc(name || `Item ${id}`);
const times = (min, max) => (min === max ? `${num(min)}×` : `${num(min)}–${num(max)}×`);

function recipeList(view, into) {
  const groups = new Map();
  for (const r of view.recipes) {
    const k = JSON.stringify([r.name, r.makes, r.min, r.max, r.reagents.map((g) => [g.id, g.count])]);
    const g = groups.get(k);
    if (g) g.who.push(r.character);
    else groups.set(k, { r, who: [r.character] });
  }
  const arrow = '<svg class="rc-arrow" viewBox="0 0 24 12" width="24" height="12"><path d="M1 6h20M16 1.5 21.5 6 16 10.5" fill="none" stroke="currentColor" stroke-width="1.5"/></svg>';
  const rows = [...groups.values()]
    .map(({ r, who }) => {
      const needs = r.reagents.length
        ? r.reagents.map((g) => `<span class="rc-chip"><em>${num(g.count)}×</em> ${itemName(g.id, g.name)}</span>`).join("")
        : '<span class="dim">no listed reagents</span>';
      const product = r.makes || r.name || "Recipe";
      const sub = r.makes && r.name && r.name !== r.makes ? `<span class="dim">${esc(r.name)}</span>` : "";
      return `<div class="rc-row"><div class="rc-in">${needs}</div>${arrow}<div class="rc-out"><span class="rc-chip out"><em>${times(r.min, r.max)}</em> <b>${esc(product)}</b></span>${sub}</div>
        <div class="rc-who">${who.map((n) => `<span>${esc(displayName(n))}</span>`).join("")}</div></div>`;
    })
    .join("");
  into.appendChild(el("h3", "section-title", "Recipes"));
  const list = el("div", "rc-list");
  list.innerHTML = rows;
  into.appendChild(list);
  if (view.recipes_matched > view.recipes.length) {
    into.appendChild(el("div", "dim", `Showing ${num(view.recipes.length)} of ${num(view.recipes_matched)} recipes.`));
  }
}

function results(view, into, sort) {
  into.innerHTML = "";
  if (!view.rows.length && !view.recipes.length) {
    into.appendChild(el("div", "empty", "No items match."));
    return;
  }
  if (view.rows.length) itemList(view, into, sort);
  if (view.recipes.length) recipeList(view, into);
}

function age(at, now) {
  if (!at) return { c: "never", text: "Never" };
  const s = Math.max(0, now - at);
  const text = s < 3600 ? `${Math.max(1, Math.round(s / 60))} min` : s < 86400 ? `${Math.round(s / 3600)} h` : `${Math.round(s / 86400)} d`;
  return { c: s < FRESH_SECS ? "fresh" : s < DAY_OLD_SECS ? "day" : "stale", text };
}

function carriers(view) {
  const p = el("div", "panel");
  p.innerHTML = "<h3>Characters</h3>";
  const now = Date.now() / 1000;
  const legend = key([
    ["Under 12 h", "--alt-4"],
    ["Under 2 days", "--alt-3"],
    ["Older", "--alt-1"],
    ["Never read", "--line2"],
  ]);
  const rows = view.characters
    .map((c) => {
      const cells = READS.map(([label, f]) => {
        const a = age(c[f], now);
        return `<div class="fr-cell ${a.c}" data-tip="${label} read|${esc(when(c[f]))}"><i></i><span>${label}</span><b>${a.text}</b></div>`;
      }).join("");
      return `<div class="fr-row"><span class="fr-name">${esc(displayName(c.name))}</span><span class="fr-n">${num(c.stacks)} <small>${c.stacks === 1 ? "item" : "items"}</small></span>${cells}</div>`;
    })
    .join("");
  const list = el("div", "fr-list");
  list.innerHTML = rows;
  p.append(legend, list);
  return p;
}

function sharedBanks(view) {
  if (!view.shared.length) return null;
  const p = el("div", "panel");
  p.innerHTML = "<h3>Warband and guild banks</h3>";
  const list = view.shared.map((s) => {
    const note = s.at ? `Last read ${when(s.at)}${s.read_by ? ` by ${displayName(s.read_by)}` : ""}` : "";
    const balance = el("span", "it-balance");
    balance.innerHTML = s.money == null ? "No balance read" : `Balance ${moneyHtml(s.money)}`;
    const held = (n) => `${num(n)}<small>${n === 1 ? "item" : "items"}</small>`;
    return tile(placeText(view, s.place, s.scope, s.account_unknown), held(s.stacks), note, { size: "xl", extra: balance, count: [s.stacks, held] });
  });
  p.appendChild(tiles(list, Math.min(list.length, 3)));
  return p;
}

export async function render(body) {
  const first = await invoke("items", { query: "", ch: null });
  body.innerHTML = "";
  if (!first.characters.length) {
    body.appendChild(el("div", "empty", "No bags or bank recorded yet."));
    return;
  }
  wireTips();
  const page = el("div", "it");
  const search = el("div", "panel");
  search.innerHTML = "<h3>Search</h3>";
  const bar = el("div", "row");
  const q = el("input");
  q.placeholder = "Item, recipe, or ID…";
  q.style.width = "260px";
  bar.appendChild(q);
  let scopeCh = null;
  let sort = "total";
  let seq = 0;
  let deb;
  let last = first;
  const show = () => results(last, out, sort);
  const runSearch = async () => {
    const mine = ++seq;
    const view = await invoke("items", { query: q.value, ch: scopeCh });
    if (mine !== seq) return;
    last = view;
    show();
  };
  const segment = (label, options, current, pick) => {
    const seg = el("div", "seg");
    seg.setAttribute("role", "group");
    seg.setAttribute("aria-label", label);
    for (const [val, text] of options) {
      const b = el("button", "", text);
      b.type = "button";
      b.setAttribute("aria-pressed", String(current() === val));
      b.onclick = () => {
        if (current() === val) return;
        pick(val);
        seg.querySelectorAll("button").forEach((x) => x.setAttribute("aria-pressed", String(x === b)));
      };
      seg.appendChild(b);
    }
    return seg;
  };
  if (state.ch) {
    bar.appendChild(
      segment("Item scope", [[null, "All characters"], [state.ch, displayName(state.ch)]], () => scopeCh, (val) => {
        scopeCh = val;
        runSearch();
      })
    );
  }
  bar.appendChild(
    segment("Sort items", [["total", "Most held"], ["value", "Most valuable"]], () => sort, (val) => {
      sort = val;
      show();
    })
  );
  const out = el("div");
  out.style.marginTop = "10px";
  search.append(bar, out);
  show();
  q.oninput = () => {
    clearTimeout(deb);
    deb = setTimeout(runSearch, 150);
  };
  const banks = sharedBanks(first);
  page.append(search, carriers(first), ...(banks ? [banks] : []));
  body.append(page);
  play(page);
  setTimeout(() => page.classList.remove("play"), 1800);
}
