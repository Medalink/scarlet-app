// Items: what every character carries in its bags and bank, searched by item name or id.
// The model comes from the `items` command.

import { invoke, el, esc, displayName, num, dateTime, moneyHtml, state } from "../lib.js";

const when = (secs) => (secs ? dateTime(secs * 1000) : "–");

function placeLabel(view, place, scope, accountUnknown) {
  if (accountUnknown) return `${place} (account unknown, ${esc(displayName(scope))})`;
  const several = view.shared.filter((s) => s.place === place).length > 1;
  return several && scope ? `${place} (${esc(scope)})` : place;
}

function holders(row, view) {
  const lines = row.holders.map((h) => {
    const parts = [];
    if (h.bags) parts.push(`${num(h.bags)} in bags`);
    if (h.bank) parts.push(`${num(h.bank)} in bank`);
    if (h.mail) parts.push(`${num(h.mail)} in mailbox`);
    if (h.auctions) parts.push(`${num(h.auctions)} in auctions`);
    return `${esc(displayName(h.character))} ${parts.join(", ")}`;
  });
  const places = row.places.map((p) => `${placeLabel(view, p.place, p.scope, p.account_unknown)} ${num(p.count)}`);
  return [...lines, ...places].join("<br>");
}

function carriers(view) {
  const p = el("div", "panel");
  p.innerHTML = "<h3>Characters</h3>";
  const rows = view.characters
    .map(
      (c) => `<tr><td>${esc(displayName(c.name))}</td><td class="num">${num(c.stacks)}</td>
        <td class="gap">${when(c.bags_at)}</td><td>${when(c.bank_at)}</td><td>${when(c.mail_at)}</td><td>${when(c.auctions_at)}</td></tr>`
    )
    .join("");
  const wrap = el("div", "scroll-x");
  const t = el("table");
  t.innerHTML = `<thead><tr><th>Character</th><th class="num">Items</th><th class="gap">Bags read</th><th>Bank read</th><th>Mailbox read</th><th>Auctions read</th></tr></thead><tbody>${rows}</tbody>`;
  wrap.appendChild(t);
  p.appendChild(wrap);
  return p;
}

function sharedBanks(view) {
  if (!view.shared.length) return null;
  const p = el("div", "panel");
  p.innerHTML = "<h3>Warband and guild banks</h3>";
  const rows = view.shared
    .map(
      (s) => `<tr><td>${placeLabel(view, s.place, s.scope, s.account_unknown)}</td><td>${s.read_by ? esc(displayName(s.read_by)) : "–"}</td>
        <td class="num">${num(s.stacks)}</td><td class="gap">${when(s.at)}</td><td class="num">${s.money == null ? "–" : moneyHtml(s.money)}</td></tr>`
    )
    .join("");
  const wrap = el("div", "scroll-x");
  const t = el("table");
  t.innerHTML = `<thead><tr><th>Place</th><th>Read by</th><th class="num">Items</th><th class="gap">Last read</th><th class="num">Balance</th></tr></thead><tbody>${rows}</tbody>`;
  wrap.appendChild(t);
  p.appendChild(wrap);
  return p;
}

function results(view, into) {
  into.innerHTML = "";
  if (!view.rows.length && !view.recipes.length) {
    into.appendChild(el("div", "empty", "No items match."));
    return;
  }
  if (view.rows.length) itemTable(view, into);
  if (view.recipes.length) recipeTable(view, into);
}

const itemName = (id, name) => esc(name || `Item ${id}`);

function recipeTable(view, into) {
  const rows = view.recipes
    .map((r) => {
      const makes = r.min === r.max ? `x${num(r.min)}` : `x${num(r.min)} to ${num(r.max)}`;
      const needs = r.reagents.length
        ? r.reagents.map((g) => `${itemName(g.id, g.name)} x${num(g.count)}`).join(", ")
        : "no listed reagents";
      return `<tr><td>${esc(r.name || "Recipe")}</td><td>${esc(displayName(r.character))}</td>
        <td>${r.makes ? esc(r.makes) : "–"} ${makes}</td><td class="gap">${needs}</td></tr>`;
    })
    .join("");
  const wrap = el("div", "scroll-x");
  const t = el("table");
  t.innerHTML = `<thead><tr><th>Recipe</th><th>Known by</th><th>Makes</th><th class="gap">Needs</th></tr></thead><tbody>${rows}</tbody>`;
  wrap.appendChild(t);
  into.appendChild(el("h3", "section-title", "Recipes"));
  into.appendChild(wrap);
  if (view.recipes_matched > view.recipes.length) {
    into.appendChild(el("div", "dim", `Showing ${num(view.recipes.length)} of ${num(view.recipes_matched)} recipes.`));
  }
}

function itemTable(view, into) {
  const rows = view.rows
    .map(
      (r) => `<tr><td>${esc(r.name || `Item ${r.id}`)} <span class="dim">${r.id}</span></td>
        <td class="num">${num(r.total)}</td><td class="num">${r.price ? `${moneyHtml(r.price.copper)} <span class="dim">${when(r.price.at)}</span>` : "–"}</td><td class="gap">${holders(r, view)}</td></tr>`
    )
    .join("");
  const wrap = el("div", "scroll-x");
  const t = el("table");
  t.innerHTML = `<thead><tr><th>Item</th><th class="num">Total</th><th class="num">Last auction price</th><th class="gap">Carried by</th></tr></thead><tbody>${rows}</tbody>`;
  wrap.appendChild(t);
  into.appendChild(wrap);
  if (view.matched > view.rows.length) {
    into.appendChild(el("div", "dim", `Showing ${num(view.rows.length)} of ${num(view.matched)}.`));
  }
}

export async function render(body) {
  const first = await invoke("items", { query: "", ch: null });
  body.innerHTML = "";
  if (!first.characters.length) {
    body.appendChild(el("div", "empty", "No bags or bank recorded yet."));
    return;
  }
  const search = el("div", "panel");
  search.innerHTML = "<h3>Search</h3>";
  const bar = el("div", "row");
  const q = el("input");
  q.placeholder = "Item, recipe, or ID…";
  q.style.width = "260px";
  bar.appendChild(q);
  let scopeCh = null;
  let seq = 0;
  let deb;
  const runSearch = async () => {
    const mine = ++seq;
    const view = await invoke("items", { query: q.value, ch: scopeCh });
    if (mine === seq) results(view, out);
  };
  if (state.ch) {
    const seg = el("div", "seg");
    seg.setAttribute("role", "group");
    seg.setAttribute("aria-label", "Item scope");
    for (const [val, label] of [[null, "All characters"], [state.ch, displayName(state.ch)]]) {
      const b = el("button", "", label);
      b.type = "button";
      b.setAttribute("aria-pressed", String(scopeCh === val));
      b.onclick = () => {
        if (scopeCh === val) return;
        scopeCh = val;
        seg.querySelectorAll("button").forEach((x) => x.setAttribute("aria-pressed", String(x === b)));
        runSearch();
      };
      seg.appendChild(b);
    }
    bar.appendChild(seg);
  }
  const out = el("div");
  out.style.marginTop = "10px";
  search.append(bar, out);
  results(first, out);
  q.oninput = () => {
    clearTimeout(deb);
    deb = setTimeout(runSearch, 150);
  };
  const banks = sharedBanks(first);
  body.append(search, carriers(first), ...(banks ? [banks] : []));
}
