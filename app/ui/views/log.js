// Log: virtualized event log with Rust-side search, code chips, date range
// and session jump.

import { invoke, el, state, clock, esc, num, highlight } from "../lib.js";

const ROW_PX = 24;
const ROWS_PER_PAGE = 500;

export async function render(body) {
  const sessionsList = await invoke("sessions", { ch: state.ch });
  body.innerHTML = "";

  const picked = window.__tlSession ?? null;
  window.__tlSession = null;

  const ui = {
    sig: "",
    total: 0,
    pages: new Map(),
    inflight: new Set(),
    sessFrom: picked ? picked.from : null,
    sessTo: picked ? picked.to : null,
  };

  const wrap = el("div", "tl-wrap");

  const ctl = el("div", "row");
  const q = el("input");
  q.placeholder = "Search events…";
  q.style.width = "220px";
  const from = el("input");
  from.type = "date";
  from.title = "From date";
  from.setAttribute("aria-label", "From date");
  const to = el("input");
  to.type = "date";
  to.title = "To date";
  to.setAttribute("aria-label", "To date");
  const sess = el("select");
  sess.setAttribute("aria-label", "Session");
  sess.innerHTML = `<option value="">Whole history</option>`;
  for (const s of sessionsList) {
    const o = el("option");
    o.value = s.index;
    o.textContent = `${new Date(s.start).toLocaleString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" })} · ${s.zones[0] || "Unknown zone"}`;
    sess.appendChild(o);
  }
  if (picked && picked.index != null) sess.value = String(picked.index);
  const clear = el("button", "btn", "Reset");
  clear.type = "button";
  ctl.append(q, from, to, sess, clear);

  const codeBar = el("div", "tl-bar");
  const activeBar = el("div", "tl-active");
  const allCodes = state.store?.codes ?? [];
  const details = el("details", "tl-codes");
  const summary = el("summary");
  const chips = el("div", "tl-codes-list");
  const all = el("button", "chip" + (state.codes.size === 0 ? " active" : ""), "All");
  all.type = "button";
  chips.appendChild(all);
  for (const [code, n] of allCodes) {
    const b = el("button", "chip" + (state.codes.has(code) ? " active" : ""), `${esc(code)}<span class="n">${num(n)}</span>`);
    b.type = "button";
    b.dataset.code = code;
    chips.appendChild(b);
  }
  details.append(summary, chips);
  codeBar.append(activeBar, details);

  function syncCodeBar() {
    const n = state.codes.size;
    summary.textContent = n
      ? `Event types (${n} active of ${num(allCodes.length)})`
      : `Event types (${num(allCodes.length)})`;
    all.classList.toggle("active", n === 0);
    chips.querySelectorAll(".chip[data-code]").forEach((c) => {
      c.classList.toggle("active", state.codes.has(c.dataset.code));
    });
    activeBar.hidden = n === 0;
    activeBar.innerHTML = "";
    if (n > 0) {
      for (const code of state.codes) {
        const b = el("button", "chip active", `${esc(code)} ×`);
        b.type = "button";
        b.title = `Remove ${code} filter`;
        b.onclick = () => {
          state.codes.delete(code);
          syncCodeBar();
          reset();
        };
        activeBar.appendChild(b);
      }
    }
  }
  syncCodeBar();

  const scroll = el("div", "tl-scroll");
  const view = el("div", "tl-abs");
  scroll.appendChild(view);
  wrap.append(ctl, codeBar, scroll);
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
      offset: p * ROWS_PER_PAGE,
      limit: ROWS_PER_PAGE,
    });
    ui.inflight.delete(key);
    if (f.sig !== ui.sig) return;
    ui.pages.set(key, res.rows);
    ui.total = res.total;
    draw();
  }

  function draw() {
    const f = { sig: ui.sig };
    const term = q.value.trim();
    if (ui.total === 0 && ui.pages.has(f.sig + ":0")) {
      view.style.height = "auto";
      view.innerHTML = `<div class="empty" style="padding:14px">No matching events.</div>`;
      return;
    }
    view.style.height = Math.max(ui.total * ROW_PX, scroll.clientHeight) + "px";
    const first = Math.max(0, Math.floor(scroll.scrollTop / ROW_PX) - 4);
    const last = first + Math.ceil(scroll.clientHeight / ROW_PX) + 8;
    let html = "";
    for (let i = first; i < Math.min(last, ui.total); i++) {
      const p = Math.floor(i / ROWS_PER_PAGE);
      const page = ui.pages.get(f.sig + ":" + p);
      if (!page) {
        fetchPage(p, f);
        continue;
      }
      const r = page[i - p * ROWS_PER_PAGE];
      const textHtml = r.text ? highlight(r.text, term) : "";
      html += `<div class="tl-row" style="top:${i * ROW_PX}px" data-code="${esc(r.code)}">
        <span class="tl-time">${clock(r.ms)}</span>
        <span class="tl-word" title="Filter by ${esc(r.word)}">${esc(r.word)}</span>
        <span class="tl-text">${textHtml}</span>
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
    fetchPage(0, { sig: ui.sig });
    draw();
  }

  let scrollFrame = 0;
  scroll.onscroll = () => {
    if (scrollFrame) return;
    scrollFrame = requestAnimationFrame(() => {
      scrollFrame = 0;
      draw();
    });
  };

  let deb;
  q.oninput = () => { clearTimeout(deb); deb = setTimeout(reset, 200); };
  from.onchange = () => { ui.sessFrom = ui.sessTo = null; sess.value = ""; reset(); };
  to.onchange = () => { ui.sessFrom = ui.sessTo = null; sess.value = ""; reset(); };
  sess.onchange = () => {
    if (sess.value === "") {
      ui.sessFrom = ui.sessTo = null;
    } else {
      const s = sessionsList.find((x) => x.index === +sess.value);
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
    syncCodeBar();
    reset();
  };

  all.onclick = () => {
    state.codes.clear();
    syncCodeBar();
    reset();
  };
  chips.onclick = (e) => {
    const c = e.target.closest(".chip[data-code]");
    if (!c) return;
    const code = c.dataset.code;
    state.codes.has(code) ? state.codes.delete(code) : state.codes.add(code);
    syncCodeBar();
    reset();
  };
  view.onclick = (e) => {
    const word = e.target.closest(".tl-word");
    if (!word) return;
    const row = word.closest(".tl-row");
    if (!row) return;
    state.codes.add(row.dataset.code);
    syncCodeBar();
    reset();
  };

  reset();
}
