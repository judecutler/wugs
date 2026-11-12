import { assistPanel } from "./assist";
import { concordancePanel } from "./concordance";
import { conflictsPanel } from "./conflicts";
import { devicesPanel } from "./devices";
import { elicitPanel } from "./elicit";
import { glossPanel } from "./gloss";
import { kitsPanel } from "./kits";
import { lexiconPanel } from "./lexicon";
import { mediaPanel } from "./media";
import type { PanelDef } from "./panel";
import { peoplePanel } from "./people";
import { searchPanel } from "./search";
import { textsPanel } from "./texts";
import { timelinePanel } from "./timeline";
import { transcribePanel } from "./transcribe";

/** Every panel a layout may name. The key is both the dockview component
 * name and the panel id: each panel exists at most once. */
export const PANELS = {
  timeline: timelinePanel,
  media: mediaPanel,
  texts: textsPanel,
  transcribe: transcribePanel,
  gloss: glossPanel,
  lexicon: lexiconPanel,
  elicit: elicitPanel,
  search: searchPanel,
  concordance: concordancePanel,
  assist: assistPanel,
  people: peoplePanel,
  conflicts: conflictsPanel,
  devices: devicesPanel,
  kits: kitsPanel,
} satisfies Record<string, PanelDef>;

export type PanelId = keyof typeof PANELS;

export const isPanelId = (name: string): name is PanelId => Object.hasOwn(PANELS, name);

/** Panels every kit offers: they look after the project and the app, not
 * one kind of linguistic work. The rest are tools a kit turns on. */
const PLATFORM: ReadonlySet<PanelId> = new Set<PanelId>(["conflicts", "devices", "kits"]);

export const isPlatformPanel = (id: string): boolean => isPanelId(id) && PLATFORM.has(id);

/** Tool panels, the ones a kit chooses among. */
export const toolPanels = (): PanelId[] => (Object.keys(PANELS) as PanelId[]).filter((id) => !PLATFORM.has(id));
