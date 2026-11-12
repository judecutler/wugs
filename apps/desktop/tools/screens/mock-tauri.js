// Stands in for the Tauri runtime so the real frontend runs in a plain
// browser. Loaded as an init script after fixtures.js; @tauri-apps/api
// reaches the backend only through window.__TAURI_INTERNALS__.
(() => {
  const fixtures = window.__WUGS_FIXTURES__ ?? {};
  const config = window.__WUGS_SCREENS__ ?? { autoOpen: true };
  const unknown = new Set();
  const calls = [];
  window.__WUGS_UNKNOWN__ = unknown;
  window.__WUGS_CALLS__ = calls;

  // Fresh state each load: the app reopens the last project from localStorage.
  localStorage.clear();
  localStorage.setItem("wugs.researcher", "demo");
  localStorage.setItem("wugs.gloss-lang", "es");
  localStorage.setItem("wugs.updates.check", "off");
  if (config.autoOpen) localStorage.setItem("wugs.lastProject", "/projects/field-trip-2026");
  if (config.theme) localStorage.setItem("wugs.theme", config.theme);

  const callbacks = new Map();
  let nextId = 1;
  const listeners = new Map();

  function transformCallback(callback, once = false) {
    const id = nextId++;
    callbacks.set(id, (data) => {
      if (once) callbacks.delete(id);
      return callback?.(data);
    });
    return id;
  }

  /** Fire a backend event from the harness: `__WUGS_EMIT__("assist-download", {...})`. */
  window.__WUGS_EMIT__ = (event, payload) => {
    for (const handler of listeners.get(event) ?? []) callbacks.get(handler)?.({ event, id: handler, payload });
  };

  // Plugin commands the shell sends that have no visible effect here.
  const plugin = {
    "plugin:event|listen": ({ event, handler }) => {
      if (!listeners.has(event)) listeners.set(event, []);
      listeners.get(event).push(handler);
      return handler;
    },
    "plugin:event|unlisten": ({ event, eventId }) => {
      const list = listeners.get(event) ?? [];
      const at = list.indexOf(eventId);
      if (at !== -1) list.splice(at, 1);
      return null;
    },
    "plugin:event|emit": ({ event, payload }) => {
      window.__WUGS_EMIT__(event, payload);
      return null;
    },
    // A dialog cancelled: the shell treats null as "nothing picked".
    "plugin:dialog|open": () => null,
    "plugin:dialog|save": () => null,
    "plugin:app|version": () => "0.1.0",
  };

  async function invoke(cmd, args = {}, _options) {
    calls.push(cmd);
    if (cmd in plugin) return plugin[cmd](args);
    if (cmd.startsWith("plugin:window|")) return null;
    if (!(cmd in fixtures)) {
      if (!unknown.has(cmd)) console.warn(`[mock-tauri] no fixture for ${cmd}`, JSON.stringify(args));
      unknown.add(cmd);
      return null;
    }
    const answer = fixtures[cmd];
    // Clone, so the app can never mutate a fixture another call returns.
    return structuredClone(typeof answer === "function" ? answer(args) : answer);
  }

  window.__TAURI_INTERNALS__ = {
    invoke,
    transformCallback,
    unregisterCallback: (id) => callbacks.delete(id),
    runCallback: (id, data) => callbacks.get(id)?.(data),
    callbacks,
    // Served by the harness (shoot.py routes it to a short silent WAV).
    convertFileSrc: (path, protocol = "asset") => `${location.origin}/__mock_${protocol}/${encodeURIComponent(path)}`,
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
    plugins: { path: { sep: "/", delimiter: ":" } },
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: (_event, id) => callbacks.delete(id) };
})();
