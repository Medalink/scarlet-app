// The header is the window's title bar: drag it to move the window, double-click it to
// maximize or restore, and the buttons at its right edge minimize, maximize and close.
import { currentWindow } from "./bridge.js";
const win = currentWindow();
const header = document.querySelector("header");

// Controls and anything else that takes a click keep it; the rest of the header drags.
const INTERACTIVE = "select, button, input, a, textarea, label, [data-no-drag]";

header.addEventListener("mousedown", (e) => {
  if (e.button !== 0 || e.target.closest(INTERACTIVE)) return;
  if (e.detail === 2) win.toggleMaximize();
  else win.startDragging();
});

document.getElementById("win-min").onclick = () => win.minimize();
document.getElementById("win-max").onclick = () => win.toggleMaximize();
document.getElementById("win-close").onclick = () => win.close();

const maxBtn = document.getElementById("win-max");
async function syncMaximized() {
  const max = await win.isMaximized();
  document.documentElement.classList.toggle("maximized", max);
  const label = max ? "Restore" : "Maximize";
  maxBtn.title = label;
  maxBtn.setAttribute("aria-label", label);
}
win.onResized(syncMaximized);
syncMaximized();
