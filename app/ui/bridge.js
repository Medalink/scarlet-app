// The webview bridge: the IPC surfaces the UI uses, called on __TAURI_INTERNALS__, the
// entry Tauri injects into every page. Call shapes mirror @tauri-apps/api 2.x, the
// package the withGlobalTauri bundle used to expose; the crate is pinned to 2.x in Cargo.toml.
const bridge = window.__TAURI_INTERNALS__;

export const invoke = (cmd, args) => bridge.invoke(cmd, args ?? {});

export async function listen(event, handler, target = { kind: "Any" }) {
  const id = await bridge.invoke("plugin:event|listen", {
    event,
    target,
    handler: bridge.transformCallback(handler),
  });
  return async () => {
    window.__TAURI_EVENT_PLUGIN_INTERNALS__?.unregisterListener(event, id);
    await invoke("plugin:event|unlisten", { event, eventId: id });
  };
}

export function currentWindow() {
  const label = bridge.metadata.currentWindow.label;
  const call = (name, args) => invoke(`plugin:window|${name}`, { label, ...args });
  return {
    startDragging: () => call("start_dragging"),
    minimize: () => call("minimize"),
    toggleMaximize: () => call("toggle_maximize"),
    isMaximized: () => call("is_maximized"),
    close: () => call("close"),
    onResized: (handler) => listen("tauri://resize", handler, { kind: "Window", label }),
  };
}
