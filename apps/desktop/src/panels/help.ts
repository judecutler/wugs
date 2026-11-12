// The Help panel (roadmap 67): the conversation of src/help/view.ts in a dock
// panel every kit offers.
import { helpView, type HelpView } from "../help/view";
import type { PanelDef, PanelView } from "./panel";

/** Opens the Feedback form; the shell sets it before any panel is made. */
let feedback: () => void = () => {};
export const setHelpFeedback = (open: () => void) => {
  feedback = open;
};

let mounted: HelpView | null = null;
/** The open panel's speaking wug and question box, for the jump and focus. */
export const helpPanelView = (): HelpView | null => mounted;

export const helpPanel: PanelDef = {
  title: "Help",
  create(): PanelView {
    const view = helpView(() => feedback());
    mounted = view;
    const element = document.createElement("div");
    element.className = "panel panel-help help-panel";
    element.append(view.element);
    return {
      element,
      render() {},
      dispose() {
        view.dispose();
        if (mounted === view) mounted = null;
      },
    };
  },
};
