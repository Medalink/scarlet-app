// Timeline: virtualized log with Rust-side search, code chips, date range
// and session jump.

import { invoke, el, state, clock, esc } from "../lib.js";

const ROW = 24; // px per row
const PAGE = 500; // rows per Rust page

export async function render(body) {
  body.innerHTML = "";

  const ui = {
    sig: "",
    total: 0,
    pages: new Map(), // sig:page -> rows
    inflight: new Set(),
    sessFrom: null,
    sessTo: null,
  };

  const wrap = el("div", "tl-wrap");

  const ctl = el("div", "row");
  const q = el("input");
  q.placeholder = "Search events…";
  q.style.width = "220px";
  const from = el("input");
  from.type = "date";
  const to = el("input");
  to.type = "date";
  const sess = el("select");
  sess.innerHTML = `<option value="">Whole history</option>`;
  for (const s of await invoke("sessions", { ch: state.ch })) {
    const o = el("option");
    o.value = s.index;
    o.textContent = `${new Date(s.start).toLocaleString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" })} · ${s.zones[0] || "?"}`;
    sess.appendChild(o);
  }
  const clear = el("button", "btn", "Reset");
  ctl.append(q, from, to, sess, clear);

  const chips = el("div");
  const all = el("span", "chip active", "ALL");
  chips.appendChild(all);
  for (const [code, n] of state.store?.codes ?? []) {
    chips.appendChild(el("span", "chip", `${esc(code)}<span class="n">${n}</span>`)).dataset.code = code;
  }

  const scroll = el("div", "tl-scroll");
  const view = el("div", "tl-abs");
  scroll.appendChild(view);
  wrap.append(ctl, chips, scroll);
  body.appendChild(wrap);

  const fromMs = () => (ui.sessFrom ?? (from.value ? new Date(from.value + "T00:00:00").getTime() : -86400e12));
  const toMs = () => (ui.sessTo ?? (to.value ? new Date(to.value + "T23:59:59").getTime() : 86400e12));

  async function fetchPage(p, f) {
    const key = f.sig + ":" + p;
    if (ui.pages.has(key) || ui.inflight.has(key)) return;
    ui.inflight.add(key);
    const res = await invoke("timeline", {
      ch: state.ch,
      q: q.value.trim(),
      codes: [...state.codes],
      fromMs: fromMs(),
      toMs: toMs(),
      offset: p * PAGE,
      limit: PAGE,
    });
    ui.inflight.delete(key);
    if (f.sig !== ui.sig) return; // a newer filter superseded this page
    ui.pages.set(key, res.rows);
    ui.total = res.total;
    draw();
  }

  function draw() {
    const f = { sig: ui.sig };
    view.style.height = Math.max(ui.total * ROW, scroll.clientHeight) + "px";
    const first = Math.max(0, Math.floor(scroll.scrollTop / ROW) - 4);
    const last = first + Math.ceil(scroll.clientHeight / ROW) + 8;
    let html = "";
    for (let i = first; i < Math.min(last, ui.total); i++) {
      const p = Math.floor(i / PAGE);
      const page = ui.pages.get(f.sig + ":" + p);
      if (!page) {
        fetchPage(p, f);
        continue;
      }
      const r = page[i - p * PAGE];
      html += `<div class="tl-row" style="top:${i * ROW}px" data-code="${esc(r.code)}">
        <span class="tl-time">${clock(r.ms)}</span>
        <span class="tl-word" title="Filter by ${esc(r.word)}">${esc(r.word)}</span>
        <span class="tl-text">${esc(r.text) || `<span class="dim">${esc(r.code)}</span>`}</span>
      </div>`;
    }
    view.innerHTML = html;
  }

  function reset() {
    ui.pages.clear();
    ui.inflight.clear();
    ui.total = 0;
    ui.sig = String(Math.random());
    scroll.scrollTop = 0;
    fetchPage(0, { sig: ui.sig }); // primes total; draw re-runs on arrival
    draw();
  }

  scroll.onscroll = draw;

  let deb;
  q.oninput = () => { clearTimeout(deb); deb = setTimeout(reset, 200); };
  from.onchange = () => { ui.sessFrom = ui.sessTo = null; sess.value = ""; reset(); };
  to.onchange = () => { ui.sessFrom = ui.sessTo = null; sess.value = ""; reset(); };
  // Session jump: fetch the session's exact bounds and filter to them.
  sess.onchange = async () => {
    if (sess.value === "") {
      ui.sessFrom = ui.sessTo = null;
    } else {
      const list = await invoke("sessions", { ch: state.ch });
      const s = list.find((x) => x.index === +sess.value);
      if (s) {
        ui.sessFrom = s.start - 1000;
        ui.sessTo = s.end + 1000;
      }
    }
    reset();
  };

  clear.onclick = () => {
    q.value = "";
    from.value = to.value = "";
    ui.sessFrom = ui.sessTo = null;
    sess.value = "";
    state.codes.clear();
    chips.querySelectorAll(".chip[data-code]").forEach((c) => c.classList.remove("active"));
    all.classList.add("active");
    reset();
  };

  all.onclick = () => {
    state.codes.clear();
    chips.querySelectorAll(".chip[data-code]").forEach((c) => c.classList.remove("active"));
    all.classList.add("active");
    reset();
  };
  chips.onclick = (e) => {
    const c = e.target.closest(".chip[data-code]");
    if (!c) return;
    const code = c.dataset.code;
    state.codes.has(code) ? state.codes.delete(code) : state.codes.add(code);
    c.classList.toggle("active");
    all.classList.toggle("active", state.codes.size === 0);
    reset();
  };
  // Click a word in a row to filter by that code.
  view.onclick = (e) => {
    const row = e.target.closest(".tl-row");
    if (!row) return;
    const code = row.dataset.code;
    state.codes.add(code);
    const c = chips.querySelector(`[data-code="${code}"]`);
    if (c) c.classList.add("active");
    all.classList.remove("active");
    reset();
  };

  reset();
}
