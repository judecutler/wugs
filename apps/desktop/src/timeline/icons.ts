// Track-header and transport icons, drawn in currentColor so they follow
// the theme. Fixed strings, never built from data.
const svg = (body: string): string =>
  `<svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">${body}</svg>`;

export const ICONS = {
  eye: svg('<path d="M1.5 8s2.4-4.5 6.5-4.5S14.5 8 14.5 8 12.1 12.5 8 12.5 1.5 8 1.5 8z"/><circle cx="8" cy="8" r="2"/>'),
  eyeOff: svg('<path d="M1.5 8s2.4-4.5 6.5-4.5S14.5 8 14.5 8 12.1 12.5 8 12.5 1.5 8 1.5 8z"/><path d="M2.5 13.5l11-11"/>'),
  lock: svg('<rect x="3.5" y="7" width="9" height="6.5" rx="1"/><path d="M5.5 7V5a2.5 2.5 0 015 0v2"/>'),
  unlock: svg('<rect x="3.5" y="7" width="9" height="6.5" rx="1"/><path d="M5.5 7V5a2.5 2.5 0 014.8-1"/>'),
  sound: svg('<path d="M2.5 6v4h2.5l3.5 3V3L5 6z"/><path d="M11 5.5a3.5 3.5 0 010 5"/>'),
  muted: svg('<path d="M2.5 6v4h2.5l3.5 3V3L5 6z"/><path d="M11 6l3 4M14 6l-3 4"/>'),
  play: svg('<path d="M4.5 3v10l8.5-5z" fill="currentColor"/>'),
  pause: svg('<path d="M5 3v10M11 3v10" stroke-width="2.5"/>'),
  start: svg('<path d="M3.5 3v10"/><path d="M12.5 3v10L5.5 8z" fill="currentColor"/>'),
  end: svg('<path d="M12.5 3v10"/><path d="M3.5 3v10l7-5z" fill="currentColor"/>'),
} as const;

export type IconName = keyof typeof ICONS;
