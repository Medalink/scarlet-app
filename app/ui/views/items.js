// Items: what every character carries in its bags and bank, searched by item name or id.
// The model comes from the `items` command.

import { invoke, el, esc, displayName, num, dateTime, moneyHtml } from "../lib.js";

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
  const t = el("table");
  t.innerHTML = `<thead><tr><th>Character</th><th class="num">Items</th><th class="gap">Bags read</th><th>Bank read</th><th>Mailbox read</th><th>Auctions read</th></tr></thead><tbody>${rows}</tbody>`;
  p.appendChild(t);
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
  const t = el("table");
  t.innerHTML = `<thead><tr><th>Place</th><th>Read by</th><th class="num">Items</th><th class="gap">Read</th><th class="num">Balance</th></tr></thead><tbody>${rows}</tbody>`;
  p.appendChild(t);
  return p;
}

function results(view, into) {
  into.innerHTML = "";
  if (!view.rows.length) {
    into.appendChild(el("div", "empty", "No items match."));
    return;
  }
  const rows = view.rows
    .map(
      (r) => `<tr><td>${esc(r.name || `Item ${r.id}`)} <span class="dim">${r.id}</span></td>
        <td class="num">${num(r.total)}</td><td class="gap">${holders(r, view)}</td></tr>`
    )
    .join("");
  const t = el("table");
  t.innerHTML = `<thead><tr><th>Item</th><th class="num">Total</th><th class="gap">Carried by</th></tr></thead><tbody>${rows}</tbody>`;
  into.appendChild(t);
  if (view.matched > view.rows.length) {
    into.appendChild(el("div", "dim", `Showing ${num(view.rows.length)} of ${num(view.matched)}.`));
  }
}

export async function render(body) {
  body.innerHTML = "";
  const first = await invoke("items", { query: "" });
  if (!first.characters.length) {
    body.appendChild(el("div", "empty", "No bags or bank recorded yet."));
    return;
  }
  const search = el("div", "panel");
  search.innerHTML = "<h3>Search</h3>";
  const q = el("input");
  q.placeholder = "Item name or id";
  q.style.width = "260px";
  const out = el("div");
  search.append(q, out);
  results(first, out);
  let seq = 0;
  q.oninput = async () => {
    const mine = ++seq;
    const view = await invoke("items", { query: q.value });
    if (mine === seq) results(view, out);
  };
  const banks = sharedBanks(first);
  body.append(search, carriers(first), ...(banks ? [banks] : []));
}
