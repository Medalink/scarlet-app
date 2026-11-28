// Every date a person reads, in the forms of DATES.md: a moment is relative until it is a week old, a calendar day is
// never relative, and both keep their exact value on hover. Nothing else formats a date for display.

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const DAY_MS = 86_400_000;

const pad = (n) => String(n).padStart(2, "0");
const noon = (ms) => new Date(ms).getHours() % 12 || 12;
const meridiem = (ms) => (new Date(ms).getHours() < 12 ? "AM" : "PM");
const midnight = (ms) => {
  const d = new Date(ms);
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
};
const daysBetween = (from, to) => Math.round((midnight(to) - midnight(from)) / DAY_MS);
const plural = (n, unit) => `${n} ${unit}${n === 1 ? "" : "s"}`;
const escape = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

/** The day as a sortable key, `2026-11-22`. For grouping and lookups, never for display. */
export const dayKey = (ms) => {
  const d = new Date(ms);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
};

/** The milliseconds of a `YYYY-MM-DD` key, at noon so a time zone shift cannot move it to another day. */
export const keyMs = (key) => Date.parse(`${key}T12:00`);

/** The Monday at midnight that starts the week of `ms`. */
export const weekStart = (ms = Date.now()) => {
  const m = new Date(midnight(ms));
  m.setDate(m.getDate() - ((m.getDay() + 6) % 7));
  return m;
};

/** `Nov 22`, with the year when it is not this one: `Nov 22 2025`. */
export const shortDay = (ms, now = Date.now()) => {
  const d = new Date(ms);
  const year = d.getFullYear() === new Date(now).getFullYear() ? "" : ` ${d.getFullYear()}`;
  return `${MONTHS[d.getMonth()]} ${d.getDate()}${year}`;
};

/** `Sat Nov 22`. */
export const weekdayDay = (ms, now = Date.now()) => `${WEEKDAYS[new Date(ms).getDay()]} ${shortDay(ms, now)}`;

/** `Nov 22 to Nov 28`. */
export const dayRange = (from, to, now = Date.now()) => `${shortDay(from, now)} to ${shortDay(to, now)}`;

/** `Nov`, for the months along a chart. */
export const monthName = (ms) => MONTHS[new Date(ms).getMonth()];

/** `8:19 PM`. */
export const clockTime = (ms) => `${noon(ms)}:${pad(new Date(ms).getMinutes())} ${meridiem(ms)}`;

/** `8:19:47 PM`. */
export const clockExact = (ms) => `${noon(ms)}:${pad(new Date(ms).getMinutes())}:${pad(new Date(ms).getSeconds())} ${meridiem(ms)}`;

/** `8 PM`, for the hours along a chart. */
export const hourName = (ms) => `${noon(ms)} ${meridiem(ms)}`;

/** `Nov 22 8:19 PM`; a time today is just `8:19 PM`. */
export const dayTime = (ms, now = Date.now()) => (daysBetween(ms, now) === 0 ? clockTime(ms) : `${shortDay(ms, now)} ${clockTime(ms)}`);

/** `Today`, `Yesterday`, else `Sat Nov 22`. */
export const dayHeading = (ms, now = Date.now()) => {
  const back = daysBetween(ms, now);
  return back === 0 ? "Today" : back === 1 ? "Yesterday" : weekdayDay(ms, now);
};

/** What the hover of any shortened date reads: `Sat Nov 22 2025 8:19:47 PM`. */
export const exactStamp = (ms) => `${WEEKDAYS[new Date(ms).getDay()]} ${MONTHS[new Date(ms).getMonth()]} ${new Date(ms).getDate()} ${new Date(ms).getFullYear()} ${clockExact(ms)}`;

/** When something happened: `just now`, `5 minutes ago`, `yesterday`, `3 days ago`, then `Nov 22`; ahead of now `in 2 hours`. */
export function ago(ms, now = Date.now()) {
  const diff = now - ms;
  const seconds = Math.floor(Math.abs(diff) / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  if (diff >= 0) {
    if (seconds < 10) return "just now";
    if (seconds < 60) return `${seconds} seconds ago`;
    if (minutes < 60) return `${plural(minutes, "minute")} ago`;
    if (hours < 24) return `${plural(hours, "hour")} ago`;
    const days = daysBetween(ms, now);
    if (days <= 1) return "yesterday";
    return days < 7 ? `${days} days ago` : shortDay(ms, now);
  }
  if (seconds < 60) return `in ${plural(Math.max(seconds, 1), "second")}`;
  if (minutes < 60) return `in ${plural(minutes, "minute")}`;
  if (hours < 24) return `in ${plural(hours, "hour")}`;
  const days = daysBetween(now, ms);
  return days < 7 ? `in ${plural(days, "day")}` : shortDay(ms, now);
}

const hover = (ms, tip, shown) => (!shown ? "" : tip ? ` data-tip="${exactStamp(ms)}"` : ` title="${exactStamp(ms)}"`);

/** A moment as a `<time>` that the app's one timer keeps current; `lead` is the copy before it, `Last read `. `tip` puts the exact value in `data-tip`, for a surface that has its own tooltip; `hover: false` leaves it out where the parent says it. */
export function stamp(ms, lead = "", { tip = false, hover: shown = true } = {}) {
  const t = document.createElement("time");
  t.dateTime = new Date(ms).toISOString();
  t.dataset.ago = String(ms);
  if (lead) t.dataset.lead = lead;
  if (shown && tip) t.dataset.tip = exactStamp(ms);
  else if (shown) t.title = exactStamp(ms);
  t.textContent = lead + ago(ms);
  return t;
}

/** `stamp` as HTML for a template. */
export const stampHtml = (ms, lead = "", { tip = false, hover: shown = true } = {}) =>
  `<time datetime="${new Date(ms).toISOString()}" data-ago="${ms}"${lead ? ` data-lead="${escape(lead)}"` : ""}${hover(ms, tip, shown)}>${escape(lead)}${ago(ms)}</time>`;

/** Redraws the relative text of every stamp under `root`. */
export function refreshStamps(root = document) {
  const now = Date.now();
  for (const t of root.querySelectorAll("[data-ago]")) {
    const text = (t.dataset.lead ?? "") + ago(Number(t.dataset.ago), now);
    if (t.textContent !== text) t.textContent = text;
  }
}
