// Settings > Website: the master switch, the website address, one switch per sharing group and per
// character, the account link and the removals. Off until the player turns it on.

import { invoke, el, bytes, num, displayName } from "../lib.js";

// What each group holds (docs/DESIGN.md, Sharing groups). Labels come from the app.
const HOLDS = {
  core: "Sessions, levels, the zones you enter, the experience preset, war mode, time played. Always sent while uploading is on.",
  xp: "Experience gains and their sources, rested state, AFK, camps.",
  quests: "Accepts, turn-ins, objectives, rewards, gossip choices, scenarios.",
  combat: "Kills, deaths and their recap, fights, bosses, casts, auras, damage totals.",
  pvp: "Honor, player kills, battlegrounds, duels.",
  movement: "Path points, distance, flights, mounts, teleports, waypoints.",
  world: "Creatures and objects you met and the map cell you stood in, NPC windows, vignettes.",
  loot: "What you looted and from what, the loot rule, rolls, bag changes.",
  economy: "Money, vendors, the auction house, mail, trades, the bank, currencies.",
  professions: "Skills, crafts, recipes, orders.",
  build: "Stats, gear, talents, spells, action bars, pets.",
  social: "Groups, guild, friends, players seen, chat counts. Other players' names are sent only with this on.",
  collections: "Legacy Challenges and points, mounts, pets, reputation, statistics.",
  client: "Client checks, addon list, UI errors, screenshots, loading screens.",
  addons: "Lines other addons write through Scarlet.",
};

const when = (t) => new Date(t * 1000).toLocaleString([], { dateStyle: "medium", timeStyle: "short" });

function sw(label, on, locked) {
  const s = el("span", "switch");
  s.innerHTML = `<span class="track"><span class="knob"></span></span><span class="lab"></span>`;
  s.querySelector(".lab").textContent = label;
  s.classList.toggle("on", on);
  if (locked) s.classList.add("locked");
  return s;
}

export async function renderUpload(body) {
  const box = el("div");
  body.appendChild(box);
  let v = await invoke("upload_get");
  let asking = null; // a group just switched off that the player must answer for
  let confirming = false; // "Delete everything" waits for a second click
  let linkCode = "";
  let linkAt = 0;
  let linked = false;
  let msg = { cls: "status", text: "" };

  const say = (text, err) => (msg = { cls: err ? "status err" : "status ok", text });
  const fail = (e) => say(String(e), true);

  async function act(fn) {
    msg = { cls: "status", text: "" };
    try {
      const r = await fn();
      if (typeof r === "string") say(r);
    } catch (e) {
      fail(e);
    }
    v = await invoke("upload_get");
    draw();
  }

  function draw() {
    box.innerHTML = "";
    const master = sw("Upload to the Scarlet website", v.enabled);
    master.onclick = () => act(() => invoke("upload_set_enabled", { on: !v.enabled }));
    const sentence = el("div", "note");
    sentence.style.marginTop = "8px";
    sentence.textContent =
      "Sends your finished play sessions to the Scarlet website, minus the groups you switch off. Nothing is sent until you turn this on, and it never affects recording or this app.";
    box.append(master, sentence);

    const addr = el("div", "field");
    addr.style.marginTop = "16px";
    addr.innerHTML = `<label>Website address</label>`;
    const addrIn = el("input");
    addrIn.value = v.server;
    addrIn.placeholder = "https://";
    addrIn.onchange = () => act(() => invoke("upload_set_server", { server: addrIn.value }));
    addr.appendChild(addrIn);
    box.appendChild(addr);

    const bar = el("div");
    bar.style.cssText = "display:flex;gap:10px;flex-wrap:wrap;margin-top:4px;align-items:center";
    const now = el("button", "btn primary", v.busy ? "Uploading…" : "Upload now");
    now.type = "button";
    now.disabled = v.busy || !v.enabled;
    now.onclick = () => act(() => invoke("upload_now"));
    const link = el("button", "btn", linked ? "Linked to your account" : "Link to my account");
    link.type = "button";
    link.disabled = !v.enabled || linked;
    link.onclick = () =>
      act(async () => {
        const r = await invoke("upload_link");
        linked = r.linked;
        linkCode = r.linked ? "" : r.code;
        linkAt = Date.now();
        return null;
      });
    bar.append(now, link);
    box.appendChild(bar);

    if (linkCode) {
      const card = el("div", "linkcode");
      const code = el("div", "code", linkCode);
      const copy = el("button", "btn", "Copy");
      copy.type = "button";
      copy.onclick = async () => {
        try {
          await navigator.clipboard.writeText(linkCode);
          copy.textContent = "Copied";
        } catch {
          copy.textContent = "Select and copy";
        }
      };
      const row = el("div", "row");
      row.append(code, copy);
      card.append(el("div", "lab", "Code"), row, el("div", "note", "Confirm it on the page that opened in your browser."));
      box.appendChild(card);
    }

    const st = el("div", msg.cls, "");
    st.textContent = msg.text || v.error || v.note || "";
    if (!msg.text && v.error) st.className = "status err";
    box.appendChild(st);

    box.appendChild(el("h3", "section-title", "Recent uploads"));
    const log = el("div", "uplog");
    if (!v.log.length) log.appendChild(el("div", "note", "Nothing uploaded from this PC yet."));
    for (const e of v.log) {
      const row = el("div", "kv");
      const k = el("div", "k");
      k.textContent = when(e.at);
      const val = el("div", "v");
      let text;
      if (e.error) text = `Failed: ${e.error}`;
      else if (!e.parts) text = "Nothing new";
      else
        text = `Sent ${num(e.parts)} ${e.parts === 1 ? "part" : "parts"} of ${num(e.sessions)} ${e.sessions === 1 ? "session" : "sessions"} for ${e.characters.map(displayName).join(", ")}`;
      text += `. ${bytes(e.sent)}`;
      if (e.repeats) text += `, checked ${num(e.repeats + 1)} times`;
      val.textContent = text + ".";
      if (e.error) val.classList.add("err");
      row.append(k, val);
      log.appendChild(row);
    }
    box.appendChild(log);

    const gl = el("div");
    gl.style.marginTop = "20px";
    for (const g of v.groups) {
      const row = el("div");
      row.style.marginBottom = "12px";
      const locked = !g.switchable;
      const s = sw(g.label, locked ? true : g.on, locked);
      if (!locked) {
        s.onclick = () => {
          asking = g.on && v.uploaded ? g.key : null;
          act(() => invoke("upload_set_group", { key: g.key, on: !g.on }));
        };
      }
      const line = el("div", "note");
      line.style.marginTop = "2px";
      line.style.marginLeft = "41px";
      line.textContent = HOLDS[g.key] || "";
      row.append(s, line);
      if (asking === g.key && !g.on) {
        const q = el("div", "note");
        q.style.marginLeft = "41px";
        q.style.marginTop = "6px";
        q.append(`Keep what the website holds for ${g.label}, or remove it? `);
        const keep = el("button", "linkbtn", "Keep");
        keep.type = "button";
        keep.style.marginTop = "0";
        keep.onclick = () => {
          asking = null;
          draw();
        };
        const rm = el("button", "linkbtn", "Remove");
        rm.type = "button";
        rm.style.marginTop = "0";
        rm.style.marginLeft = "12px";
        rm.onclick = () => {
          const key = g.key;
          asking = null;
          act(() => invoke("upload_remove_group", { key }));
        };
        q.append(keep, rm);
        row.appendChild(q);
      }
      gl.appendChild(row);
    }
    box.appendChild(gl);

    if (v.characters.length) {
      const cl = el("div");
      cl.style.marginTop = "20px";
      for (const c of v.characters) {
        const row = el("div");
        row.style.marginBottom = "12px";
        const s = sw(displayName(c.name), c.on);
        s.onclick = () => act(() => invoke("upload_set_character", { name: c.name, on: !c.on }));
        const line = el("div", "note");
        line.style.marginTop = "2px";
        line.style.marginLeft = "41px";
        line.textContent = c.last_sync
          ? `Last upload ${when(c.last_sync)}. ${num(c.sessions)} ${c.sessions === 1 ? "session" : "sessions"}. ${bytes(c.last_sent)} sent in the last sync, ${bytes(c.total_sent)} in all.`
          : "Not uploaded yet.";
        row.append(s, line);
        cl.appendChild(row);
      }
      box.appendChild(cl);
    }

    const danger = el("div");
    danger.style.cssText = "display:flex;gap:10px;flex-wrap:wrap;margin-top:20px;align-items:center";
    if (confirming) {
      const del = el("button", "btn", "Delete from the website");
      del.type = "button";
      del.onclick = () => {
        confirming = false;
        act(() => invoke("upload_delete_all"));
      };
      const cancel = el("button", "linkbtn", "Cancel");
      cancel.type = "button";
      cancel.style.marginTop = "0";
      cancel.onclick = () => {
        confirming = false;
        draw();
      };
      danger.append(del, cancel);
    } else {
      const all = el("button", "btn", "Delete everything I uploaded");
      all.type = "button";
      all.onclick = () => {
        confirming = true;
        draw();
      };
      danger.appendChild(all);
    }
    box.appendChild(danger);
    if (confirming) {
      box.appendChild(
        el("div", "note", "This removes every part, every row and the characters you uploaded. Your data in this app stays.")
      );
    }
  }

  draw();
  try {
    linked = await invoke("upload_linked");
    draw();
  } catch {
    /* the link state is a convenience; the button still works */
  }
  // While an upload runs, follow it until it ends; while a code is shown, notice when it is confirmed.
  const timer = setInterval(async () => {
    if (!box.isConnected) return clearInterval(timer);
    if (linkCode && !linked && Date.now() - linkAt < 10 * 60 * 1000) {
      try {
        if (await invoke("upload_linked")) {
          linked = true;
          linkCode = "";
          say("Linked to your account.");
          draw();
        }
      } catch {
        /* checked again on the next tick */
      }
    }
    if (!v.busy) return;
    v = await invoke("upload_get");
    draw();
  }, 1500);
}
