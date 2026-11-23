import { el, lineChart, wireLineCharts, esc } from "../lib.js";

export function panel(title, inner) {
  const p = el("div", "panel chart-panel");
  p.innerHTML = `<h3>${title}</h3>`;
  if (typeof inner === "string") p.insertAdjacentHTML("beforeend", inner);
  else p.appendChild(inner);
  return p;
}

const STAT_LABEL = {
  ilvlall: "Item level (all)",
  ilvleq: "Item level (equipped)",
  str: "Strength",
  agi: "Agility",
  sta: "Stamina",
  int: "Intellect",
  spi: "Spirit",
  hp: "Health",
  power: "Power",
  armor: "Armor",
  ap: "Attack power",
  rap: "Ranged attack power",
  sp: "Spell power",
  crit: "Crit chance",
  scrit: "Spell crit",
  rcrit: "Ranged crit",
  dodge: "Dodge",
  parry: "Parry",
  block: "Block",
  def: "Defense",
  haste: "Haste",
  mh: "Main-hand DPS",
  oh: "Off-hand DPS",
};

export const statLabel = (s) => STAT_LABEL[s.key] || s.key;

export function pair(...panels) {
  const row = el("div", "two-col");
  row.append(...panels);
  return row;
}

export function picker(title, list, label, color) {
  if (!list.length) return null;
  const holder = el("div");
  const sel = el("select");
  sel.setAttribute("aria-label", title);
  sel.innerHTML = list.map((s) => `<option value="${esc(s.key)}">${esc(label(s))}</option>`).join("");
  const chart = el("div");
  const draw = () => {
    const s = list.find((x) => x.key === sel.value);
    chart.innerHTML = s ? lineChart(s.series, color) : "";
    wireLineCharts(chart);
  };
  sel.addEventListener("change", draw);
  draw();
  holder.append(sel, chart);
  return panel(title, holder);
}
