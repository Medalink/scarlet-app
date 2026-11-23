// Feats: a character's head, the three feats closest to their next tier, and every feat it can hold by collection.
// With uploading off the counts are the ones the recording can hold; with it on they are the website's.

import { invoke, el, elText, state, num, displayName, tile, tiles, tickRing, collectionRow, linkRow } from "../lib.js";
import { BY_KEY, CATALOG } from "../feats-catalog.js";
import { TIER_NAMES } from "../glyphs.js";
import { crest, pips, tierChip, cardList, cardCrest, countText, amount, withCatalog } from "../featcard.js";
import { playToasts, stopToasts } from "../feat-toast.js";
import { linkPrompt } from "../link.js";

const TIERS = [1, 2, 3, 4, 5];
const COLLECTIONS = [...new Set(CATALOG.feats.map((f) => f.collection))];
const SCORE_MAX = CATALOG.feats.filter((f) => f.scope === "general" && !f.economy).length * TIERS.length;

let expandNext = false;

function localCard(f) {
  const cat = BY_KEY.get(f.key);
  return {
    key: f.key,
    name: cat.name,
    tier: f.tier,
    tier_name: TIER_NAMES[f.tier] || "",
    glyph: cat.glyph,
    family: cat.family,
    collection: cat.collection,
    crest: cat.crest,
    counted: countText(cat, f.value),
    to_next: f.left == null ? "Legend" : `${amount(cat, f.left)} to ${TIER_NAMES[f.tier + 1]}`,
    progress: f.progress,
    value: f.value,
  };
}

function fromLocal(e) {
  const feats = e.feats.map(localCard);
  const byKey = new Map(feats.map((f) => [f.key, f]));
  return {
    name: e.name,
    held: e.held,
    total: e.total,
    tiers: e.tiers,
    score: e.score,
    scoreMax: e.score_max,
    elsewhere: e.elsewhere,
    feats,
    closest: e.closest.map((k) => byKey.get(k)),
    local: true,
  };
}

function siteCard(raw) {
  const feat = withCatalog(raw);
  const cat = BY_KEY.get(feat.key);
  return {
    ...feat,
    tier_name: TIER_NAMES[feat.tier] || "",
    counted: feat.counted ?? (cat && feat.value != null ? countText(cat, feat.value) : null),
    to_next: feat.to_next ?? null,
    value: feat.value ?? null,
  };
}

function fromSite(entry) {
  const count = (tier) => entry.tiers.find((t) => String(t.name).toLowerCase() === TIER_NAMES[tier].toLowerCase())?.count ?? 0;
  return {
    name: entry.name,
    held: entry.held,
    total: entry.total,
    tiers: TIERS.map(count),
    score: entry.score ?? null,
    scoreMax: SCORE_MAX,
    elsewhere: 0,
    feats: [...entry.earned, ...entry.unearned].map(siteCard).filter((f) => f.collection),
    closest: (entry.top ?? []).map(siteCard),
    news: entry.new ?? [],
    local: false,
  };
}

function accountFeat(cat, f) {
  const toNext = f.next == null ? (cat.account === "max" ? null : "Legend") : `${amount(cat, Math.max(0, f.next - f.value))} to ${TIER_NAMES[f.tier + 1]}`;
  return {
    key: cat.key,
    name: cat.name,
    tier: f.tier,
    tier_name: TIER_NAMES[f.tier] || "",
    glyph: cat.glyph,
    family: cat.family,
    collection: cat.collection,
    crest: "account",
    thresholds: cat.account_thresholds,
    counted: f.counted ?? countText(cat, f.value) + (cat.account === "sum" ? " on the account" : ""),
    to_next: [f.to_next ?? toNext, f.holder ? `Held by ${f.holder}` : null].filter(Boolean).join(" · ") || null,
    progress: f.progress,
    value: f.value,
    next: f.next,
    prev: f.prev,
  };
}

function fromLocalAccount(a) {
  const feats = a.feats.map((f) => accountFeat(BY_KEY.get(f.key), f));
  return { name: "Account", crest: "account", held: a.held, total: a.total, tiers: a.tiers, score: null, feats, local: true };
}

function fromSiteAccount(a) {
  const feats = a.cards.filter((c) => BY_KEY.get(c.key)).map((c) => accountFeat(BY_KEY.get(c.key), c));
  const byKey = new Map(feats.map((f) => [f.key, f]));
  const news = (a.new ?? []).filter((n) => byKey.has(n.key)).map((n) => accountNews(byKey.get(n.key), n.from ?? 0));
  return { name: "Account", crest: "account", held: a.held, total: a.total, tiers: TIERS.map((t) => a.tiers?.[t] ?? 0), score: null, feats, news, local: false };
}

function accountNews(f, from) {
  return {
    key: f.key,
    name: f.name,
    icon: f.glyph,
    crest: "account",
    account: true,
    from,
    from_name: TIER_NAMES[from] || "",
    tier: f.tier,
    tier_name: f.tier_name,
    counted: f.counted,
    progress: f.progress ?? 0,
  };
}

function localNews(storeKey, feats, account = false) {
  let seen = null;
  try {
    seen = JSON.parse(localStorage.getItem(storeKey));
  } catch {}
  const now = Object.fromEntries(feats.map((f) => [f.key, f.tier]));
  try {
    localStorage.setItem(storeKey, JSON.stringify(now));
  } catch {}
  if (!seen) return [];
  return feats
    .filter((f) => f.tier > (seen[f.key] ?? 0))
    .map((f) => ({
      ...accountNews(f, seen[f.key] ?? 0),
      crest: f.crest,
      account,
      progress: f.progress ?? 0,
    }));
}

function head(model) {
  const box = el("div", "feats-head");
  const ring = el("div", "sc-panel feats-ring");
  const inner = el("span", "", `<b class="sc-num" style="font-size:40px">${num(model.held)}</b><br><span class="sc-label">of ${num(model.total)}</span>`);
  const text = el("div");
  text.append(elText("h2", "sc-h3", `${displayName(model.name)} holds ${num(model.held)} ${model.held === 1 ? "feat" : "feats"}`));
  ring.append(tickRing(model.total ? model.held / model.total : 0, { size: 112, inner }), text);
  const counts = tiles(
    [...TIERS].reverse().map((t) => tile(TIER_NAMES[t], num(model.tiers[t - 1]), "", { size: "md", extra: crest("kills", t, { size: 28, scope: model.crest ?? "character" }) })),
    5,
  );
  box.append(ring, counts);
  if (model.score != null) {
    const score = el("div", "sc-panel");
    score.append(elText("p", "sc-label", "Feat score"), el("p", "sc-num sc-num-md", `${num(model.score)}<small> of ${num(model.scoreMax)}</small>`));
    box.append(score);
  } else {
    box.style.gridTemplateColumns = "minmax(0, 1fr) minmax(0, 2fr)";
  }
  return box;
}

function ladder(feat) {
  const cat = BY_KEY.get(feat.key);
  const list = el("div", "feat-ladder");
  if (!cat) return list;
  for (const t of [...TIERS].reverse()) {
    const need = (feat.thresholds ?? cat.thresholds)[t - 1];
    const row = el("div", `rung t${t}${feat.tier >= t ? " got" : ""}`);
    const fill = feat.value == null ? (feat.tier >= t ? 1 : 0) : Math.min(1, feat.value / need);
    const bar = el("i");
    bar.style.setProperty("--w", `${fill * 100}%`);
    row.append(tierChip(t, TIER_NAMES[t]), elText("span", "", countText(cat, need)), bar);
    list.append(row);
  }
  return list;
}

function detail(feat) {
  const box = el("aside", "sc-panel feat-detail");
  if (!feat) {
    box.append(elText("p", "sc-sub", "Pick a feat."));
    return box;
  }
  const top = el("div", "lead-feat");
  top.append(cardCrest(feat, 104));
  const text = el("div");
  const name = el("p", "sc-feat-n");
  name.append(feat.name);
  if (feat.tier) name.append(tierChip(feat.tier, feat.tier_name || TIER_NAMES[feat.tier]));
  text.append(name);
  const counted = feat.counted ?? feat.detail;
  if (counted) text.append(elText("p", "sc-feat-d", counted));
  if (feat.to_next) text.append(elText("p", "sc-feat-to", feat.to_next));
  top.append(text);
  box.append(top, ladder(feat));
  return box;
}

function wall(model) {
  const section = el("section", "feats-wall");
  section.append(elText("h2", "sc-h", `Every feat ${displayName(model.name).split(" ")[0]} can hold`));
  const controls = el("div", "feats-controls");
  const search = el("input");
  search.placeholder = "Find a feat";
  search.setAttribute("aria-label", "Find a feat");
  const tierBar = el("div", "chips");
  const collBar = el("div", "chips");
  controls.append(search, tierBar, collBar);
  const layout = el("div", "feats-layout");
  const rows = el("div", "feats-rows");
  const side = el("div", "feats-side");
  layout.append(rows, side);
  section.append(controls, layout);

  const have = new Set(model.feats.map((f) => f.collection));
  const collections = COLLECTIONS.filter((c) => have.has(c));
  const filter = { tier: "all", collection: "all", text: "", picked: model.closest[0]?.key ?? null };
  const chip = (bar, label, on, pick) => {
    const b = elText("button", `sc-chipbtn${on ? " on" : ""}`, label);
    b.type = "button";
    b.onclick = pick;
    bar.append(b);
  };
  const draw = () => {
    tierBar.replaceChildren();
    collBar.replaceChildren();
    const tierChoices = [["all", "All tiers"], ["none", "Not earned"], ...TIERS.map((t) => [t, TIER_NAMES[t]])];
    for (const [value, label] of tierChoices) chip(tierBar, label, filter.tier === value, () => ((filter.tier = value), draw()));
    chip(collBar, `All ${model.feats.length}`, filter.collection === "all", () => ((filter.collection = "all"), draw()));
    for (const c of collections) {
      const n = model.feats.filter((f) => f.collection === c).length;
      chip(collBar, `${c} ${n}`, filter.collection === c, () => ((filter.collection = c), draw()));
    }
    const wanted = (f) =>
      (filter.tier === "all" || (filter.tier === "none" ? f.tier === 0 : f.tier === filter.tier)) &&
      (filter.collection === "all" || f.collection === filter.collection) &&
      f.name.toLowerCase().includes(filter.text);
    rows.replaceChildren();
    for (const c of collections) {
      const feats = model.feats.filter((f) => f.collection === c);
      const shown = feats.filter(wanted);
      if (!shown.length) continue;
      const row = collectionRow(c, shown, {
        draw: { crest: (f) => cardCrest(f, 48), pips },
        picked: filter.picked,
        onPick: (f) => ((filter.picked = f.key), draw()),
        all: feats,
      });
      rows.append(row);
    }
    if (!rows.firstChild) rows.append(elText("p", "sc-empty", "No feats match."));
    side.replaceChildren(detail(model.feats.find((f) => f.key === filter.picked)));
  };
  search.oninput = () => ((filter.text = search.value.trim().toLowerCase()), draw());
  draw();
  return section;
}

function accountBlock(model) {
  const block = el("section", "feats-block feats-account");
  block.append(head(model));
  const layout = el("div", "feats-layout");
  const rows = el("div", "feats-rows");
  const side = el("div", "feats-side");
  layout.append(rows, side);
  let picked = (model.feats.find((f) => f.tier > 0) ?? model.feats[0])?.key ?? null;
  const draw = () => {
    const onPick = (f) => ((picked = f.key), draw());
    rows.replaceChildren(collectionRow("Account", model.feats, { draw: { crest: (f) => cardCrest(f, 48) }, picked, onPick }));
    side.replaceChildren(detail(model.feats.find((f) => f.key === picked)));
  };
  draw();
  block.append(layout);
  return block;
}

function characterBlock(model, single) {
  const block = el("section", "feats-block");
  if (!single) block.append(elText("h3", "section-title", displayName(model.name)));
  block.append(head(model));
  if (model.closest.length) {
    block.append(elText("h2", "sc-h feats-h", "Closest to the next tier"), cardList(model.closest, true));
  }
  const reveal = () => {
    if (block.querySelector(".feats-wall")) return;
    button?.remove();
    const drawn = wall(model);
    if (tail) tail.before(drawn);
    else block.append(drawn);
  };
  let button = null;
  let tail = null;
  if (model.local && model.elsewhere) {
    const more = `${num(model.elsewhere)} more ${model.elsewhere === 1 ? "feat is" : "feats are"} counted on the website`;
    tail = linkRow(more, "Website settings", () => (location.hash = "settings/website"));
    block.append(tail);
  }
  if (single) reveal();
  else {
    button = elText("button", "sc-btn quiet small", "Every feat");
    button.type = "button";
    button.onclick = reveal;
    block.append(button);
  }
  return { block, reveal };
}

function drawCharacters(box, models, account) {
  const wanted = state.ch;
  const entries = wanted ? models.filter((c) => c.name === wanted) : models;
  if (!entries.length && !account) {
    box.append(elText("p", "empty", wanted ? "No feats counted for this character yet." : "No feats counted yet."));
    return;
  }
  const single = Boolean(wanted);
  const accountView = account ? accountBlock(account) : null;
  if (accountView) box.append(accountView);
  const views = entries.map((entry) => characterBlock(entry, single));
  for (const { block } of views) box.append(block);
  if (!single && expandNext && views.length) views[0].reveal();
  expandNext = false;

  const at = Math.max(0, entries.findIndex((entry) => entry.news?.length));
  const entry = entries[at];
  const news = [...(entry?.news ?? []), ...(account?.news ?? [])];
  if (!news.length) return;
  playToasts(
    entry?.name ?? account.name,
    news,
    () => {
      if (!entry?.news?.length) return accountView.scrollIntoView({ block: "start" });
      if (single) return views[at].reveal();
      expandNext = true;
      const select = document.querySelector("#char");
      select.value = entry.name;
      select.dispatchEvent(new Event("change"));
    },
    { local: (entry ?? account).local },
  );
}

async function fetchModels(ch) {
  let answer;
  try {
    answer = await invoke("feats_get", { ch });
  } catch (e) {
    answer = { state: "error", message: String(e) };
  }
  if (answer.state === "error") return { error: answer.message };
  const local = answer.state === "local";
  const account = local ? answer.account && fromLocalAccount(answer.account) : answer.account && fromSiteAccount(answer.account);
  return { local, models: answer.characters.map(local ? fromLocal : fromSite), account: account || null };
}

export async function closestFor(name, n = 3) {
  const got = await fetchModels(name);
  return got.models?.find((m) => m.name === name)?.closest.slice(0, n) ?? [];
}

async function load(box) {
  const got = await fetchModels(state.ch);
  if (!box.isConnected) return;
  box.innerHTML = "";
  if (got.error) {
    box.append(elText("p", "status err", got.error));
    return;
  }
  if (got.local) {
    const wanted = state.ch;
    for (const m of got.models.filter((x) => !wanted || x.name === wanted)) m.news = localNews(`scarlet.featSeen.${m.name}`, m.feats);
    if (got.account) got.account.news = localNews("scarlet.featSeen.account", got.account.feats, true);
  }
  drawCharacters(box, got.models, got.account);
  const row = await linkPrompt("feats");
  if (row && box.isConnected) box.append(row);
}

export async function render(body) {
  stopToasts();
  body.innerHTML = "";
  const box = el("div", "feats-view");
  box.appendChild(el("p", "loading", "Loading feats…"));
  body.append(box);
  load(box).catch((e) => {
    box.innerHTML = "";
    box.append(elText("p", "status err", String(e?.message ?? e)));
  });
}
