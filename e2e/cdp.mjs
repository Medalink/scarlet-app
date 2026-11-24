// A small Chrome DevTools client for the app under test. The app starts only through hidden-desktop.ps1, which runs
// it on a Windows desktop of its own, so no window appears on the desktop in use.
import { spawn } from "node:child_process";
import fs from "node:fs";
import net from "node:net";
import path from "node:path";

const LAUNCHER = path.join(import.meta.dirname, "hidden-desktop.ps1");

const freePort = () =>
  new Promise((resolve) => {
    const server = net.createServer();
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
  });

const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const ATTACH_MS = 15000;
const ANSWER_MS = 2000;
const CLOSE_MS = 10000;

const ownEnvironment = (home, port, reducedMotion) => {
  const inherited = Object.fromEntries(Object.entries(process.env).filter(([name]) => !/^(SCARLET_OUT|SCARLET_SETTINGS|WEBVIEW2_.*)$/i.test(name)));
  return { ...inherited, SCARLET_HOME: home, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --lang=en-US${reducedMotion ? " --force-prefers-reduced-motion" : ""}` };
};

/**
 * Starts `exe` on a desktop of its own with `home` as its SCARLET_HOME (`reducedMotion` skips the entrances, so numbers
 * are final when a check reads them) and returns the page's handle:
 * `evaluate(expression)`, `until(expression, what, ms)`, `screenshot(file, { full })`, `problems`, `close()`, `pid` and
 * `exitCode()` (the launcher's, null while it runs; 3 means a window of the app was found on the desktop in use).
 * `close()` clicks the window's close button and returns the launcher's exit code, or null when the app had not
 * ended after 10 s and the launcher was killed. The launcher ends the app when this process is gone. An error thrown
 * while starting carries the launcher's exit code as `launcherExit`.
 */
export async function launch({ exe, home, reducedMotion = false }) {
  const port = await freePort();
  const launcher = spawn("pwsh", ["-NoProfile", "-File", LAUNCHER, "-Parent", String(process.pid), "-Exe", exe], {
    env: ownEnvironment(home, port, reducedMotion),
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  let exitCode = null;
  let stderr = "";
  const exited = new Promise((resolve) => {
    launcher.on("exit", (code) => {
      exitCode = code;
      resolve(code);
    });
    launcher.on("error", (error) => {
      stderr += `pwsh did not start: ${error.message}`;
      exitCode = -1;
      resolve(exitCode);
    });
  });
  const startFailed = (message) => Object.assign(new Error(message), { launcherExit: exitCode });
  launcher.stderr.on("data", (d) => (stderr += d));
  const killLauncher = () => {
    if (exitCode === null) launcher.kill();
  };
  process.on("exit", killLauncher);

  const pid = await new Promise((resolve, reject) => {
    launcher.stdout.on("data", (d) => {
      const m = /pid (\d+)/.exec(String(d));
      if (m) resolve(Number(m[1]));
    });
    exited.then((code) => reject(startFailed(`the launcher ended with exit code ${code} before the app started${stderr ? `: ${stderr.trim()}` : ""}`)));
  });

  let socket;
  try {
    let page = null;
    const giveUp = performance.now() + ATTACH_MS;
    while (!page && exitCode === null && performance.now() < giveUp) {
      page = await fetch(`http://127.0.0.1:${port}/json`, { signal: AbortSignal.timeout(ANSWER_MS) })
        .then((answer) => answer.json())
        .then((targets) => targets.find((t) => t.type === "page") ?? null, () => null);
      if (!page) await pause(25);
    }
    if (!page) throw startFailed(`the app showed no page to attach to within ${ATTACH_MS / 1000} s (launcher exit code ${exitCode})`);
    socket = new WebSocket(page.webSocketDebuggerUrl);
    await new Promise((resolve, reject) => {
      const late = setTimeout(() => reject(startFailed(`the DevTools socket did not open within ${ANSWER_MS / 1000} s`)), ANSWER_MS);
      socket.onopen = () => {
        clearTimeout(late);
        resolve();
      };
      socket.onerror = () => {
        clearTimeout(late);
        reject(startFailed("the DevTools socket did not open"));
      };
    });
  } catch (error) {
    killLauncher();
    throw error;
  }

  const problems = [];
  const waiting = new Map();
  let next = 0;
  socket.onmessage = (message) => {
    const msg = JSON.parse(message.data);
    if (msg.id) {
      waiting.get(msg.id)?.(msg);
      waiting.delete(msg.id);
      return;
    }
    if (msg.method === "Runtime.exceptionThrown") {
      const d = msg.params.exceptionDetails;
      problems.push(`exception: ${d.exception?.description ?? d.text}`);
    } else if (msg.method === "Runtime.consoleAPICalled" && msg.params.type === "error") {
      problems.push(`console error: ${msg.params.args.map((a) => a.value ?? a.description).join(" ")}`);
    } else if (msg.method === "Log.entryAdded" && msg.params.entry.level === "error") {
      problems.push(`log error: ${msg.params.entry.text} ${msg.params.entry.url ?? ""}`.trim());
    } else if (msg.method === "Network.loadingFailed" && !msg.params.canceled) {
      problems.push(`request failed: ${msg.params.errorText}`);
    }
  };
  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      const id = ++next;
      const timer = setTimeout(() => reject(new Error(`${method} got no answer in 10 s`)), 10000).unref();
      waiting.set(id, (msg) => {
        clearTimeout(timer);
        resolve(msg);
      });
      socket.send(JSON.stringify({ id, method, params }));
    });
  for (const domain of ["Runtime", "Log", "Network", "Page"]) await send(`${domain}.enable`);

  const evaluate = async (expression) => {
    const { result, error } = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
    if (error) throw new Error(`evaluate failed: ${error.message}`);
    if (result.exceptionDetails) throw new Error(`evaluate threw: ${result.exceptionDetails.exception?.description ?? result.exceptionDetails.text}`);
    return result.result.value;
  };

  const until = async (expression, what, ms = 5000) => {
    const end = performance.now() + ms;
    let seen;
    for (;;) {
      try {
        seen = await evaluate(expression);
        if (seen) return seen;
      } catch (error) {
        seen = error.message;
      }
      if (performance.now() > end) {
        const page = await evaluate(`document.querySelector("#body")?.innerText.replace(/\\s+/g, " ").slice(0, 300)`).catch(() => "");
        throw new Error(`${what}\n  not true after ${ms} ms\n  last value ${JSON.stringify(seen)?.slice(0, 200)}\n  page text  ${JSON.stringify(page)}`);
      }
      await pause(20);
    }
  };

  const screenshot = async (file, { full = false } = {}) => {
    let params = { format: "png" };
    if (full) {
      const { result: metrics } = await send("Page.getLayoutMetrics");
      const { width, height } = metrics.cssContentSize ?? metrics.contentSize;
      params = { format: "png", captureBeyondViewport: true, clip: { x: 0, y: 0, width, height: Math.min(height, 16000), scale: 1 } };
    }
    const { result, error } = await send("Page.captureScreenshot", params);
    if (!result?.data) throw new Error(`screenshot failed: ${JSON.stringify(error)}`);
    fs.writeFileSync(file, Buffer.from(result.data, "base64"));
  };

  const close = async () => {
    evaluate(`document.querySelector("#win-close").click()`).catch(() => {});
    const code = await Promise.race([exited, pause(CLOSE_MS).then(() => "late")]);
    if (code === "late") {
      killLauncher();
      await exited;
    }
    socket.close();
    process.off("exit", killLauncher);
    return code === "late" ? null : code;
  };

  return { evaluate, until, screenshot, problems, close, pid, exitCode: () => exitCode };
}
