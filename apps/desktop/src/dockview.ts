// dockview-core 8.9's ESM entry ships without its stylesheet; the UMD
// bundle carries it and injects it on load, so the shell loads that one.
export { createDockview, themeLightSpaced } from "dockview-core/dist/dockview-core.js";
// From the same bundle as createDockview: the drag in progress lives in that bundle's own singleton.
export { getPaneData, getPanelData } from "dockview-core/dist/dockview-core.js";
export type { DockviewApi, IContentRenderer, SerializedDockview } from "dockview-core";
