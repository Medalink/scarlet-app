// Activity heatmap: events per weekday x hour, or per calendar day.

import { invoke, el, num, state } from "../lib.js";

const DAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

function shade(count, max) {
  if (!max || !count) return "";
  const t = Math.pow(count / max, 0.6);
  const r = Math.round(35 + t * 220);
  const g = Math.round(35 + t * 10);
  const b = Math.round(40 + t * 45);
  return `background:rgb(${r},${g},${b})`;
}

export async function render(body) {
  body.innerHTML = "";
  const data = await invoke("heatmap", { ch: state.ch });
  let mode = sessionStorage.getItem("hm-mode") || "week";

  const bar = el("div", "row");
  const bw = el("button", "btn" + (mode === "week" ? " on" : ""), "Weekday × hour");
  const bd = el("button", "btn" + (mode === "day" ? " on" : ""), "Calendar day");
  bar.append(bw, bd);
  body.appendChild(bar);

  const holder = el("div");
  holder.style.marginTop = "14px";
  body.appendChild(holder);

  function draw() {
    holder.innerHTML = "";
    if (mode === "week") {
      const g = el("div", "hm-grid");
      g.appendChild(el("div", "hm-lab"));
      for (let h = 0; h < 24; h += 1) {
        g.appendChild(el("div", "hm-toplab", h % 4 === 0 ? String(h).padStart(2, "0") : ""));
      }
      for (let d = 0; d < 7; d++) {
        g.appendChild(el("div", "hm-lab", DAYS[d]));
        for (let h = 0; h < 24; h++) {
          const c = data.week[d * 24 + h] || 0;
          g.appendChild(el("div", "hm-cell", "")).style.cssText = shade(c, data.max_week);
        }
      }
      holder.appendChild(g);
      holder.insertAdjacentHTML(
        "beforeend",
        `<div class="note">Darker means more events. Peak: ${num(data.max_week)} events in an hour.</div>`
      );
    } else {
      if (!data.days.length) {
        holder.innerHTML = `<div class="empty">No events yet.</div>`;
        return;
      }
      for (const [day, count] of data.days) {
        const r = el("div", "day-row");
        r.innerHTML = `<div>${day}</div>
          <div class="day-track"><div class="day-fill" style="width:${(count / (data.max_day || 1)) * 100}%"></div></div>
          <div class="bar-value">${num(count)}</div>`;
        holder.appendChild(r);
      }
    }
  }

  bw.onclick = () => { mode = "week"; sessionStorage.setItem("hm-mode", mode); bw.classList.add("on"); bd.classList.remove("on"); draw(); };
  bd.onclick = () => { mode = "day"; sessionStorage.setItem("hm-mode", mode); bd.classList.add("on"); bw.classList.remove("on"); draw(); };
  draw();
}
