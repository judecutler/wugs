// dockview-core 8.9's ESM entry ships without its stylesheet; the UMD
// bundle carries it and injects it on load, so the shell loads that one.
export { createDockview, themeLight } from "dockview-core/dist/dockview-core.js";
export type { DockviewApi, IContentRenderer, SerializedDockview } from "dockview-core";
