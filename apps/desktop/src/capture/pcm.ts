// Pure helpers for in-app recording: no DOM, no Tauri.

export type Level = { peak: number; rms: number };

const clamp1 = (x: number) => (x > 1 ? 1 : x < -1 ? -1 : x);

/** Interleave per-channel float blocks into little-endian PCM16 bytes. */
export function floatToPcm16(channels: readonly Float32Array[]): Uint8Array {
  const first = channels[0];
  if (first === undefined) return new Uint8Array(0);
  const frames = first.length;
  const out = new DataView(new ArrayBuffer(frames * channels.length * 2));
  let at = 0;
  for (let i = 0; i < frames; i++) {
    for (const ch of channels) {
      const x = ch[i] ?? 0;
      out.setInt16(at, Number.isNaN(x) ? 0 : Math.round(clamp1(x) * 32767), true);
      at += 2;
    }
  }
  return new Uint8Array(out.buffer);
}

/** Peak and RMS over every channel of a block. */
export function levelOf(channels: readonly Float32Array[]): Level {
  let peak = 0;
  let sum = 0;
  let n = 0;
  for (const ch of channels) {
    for (const x of ch) {
      const a = Math.abs(x);
      if (a > peak) peak = a;
      sum += x * x;
    }
    n += ch.length;
  }
  return { peak: Math.min(peak, 1), rms: n === 0 ? 0 : Math.sqrt(sum / n) };
}

/** Linear amplitude to a 0..1 meter fill on a dB scale. */
export function meterFill(amplitude: number, floorDb = -60): number {
  if (!(amplitude > 0)) return 0;
  return Math.min(1, Math.max(0, 1 - Math.log10(Math.min(amplitude, 1)) * 20 / floorDb));
}

const pad = (n: number) => String(n).padStart(2, "0");

/** "Recording 2026-11-09 08:15" in local time. */
export function defaultLabel(now: Date): string {
  return `Recording ${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(now.getHours())}:${pad(now.getMinutes())}`;
}

/** "0:07", "12:30", "1:02:03". */
export function formatElapsed(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const s = total % 60;
  const m = Math.floor(total / 60) % 60;
  const hours = Math.floor(total / 3600);
  return hours > 0 ? `${hours}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

/** What a failed getUserMedia means to a person, from the DOMException name. */
export function micErrorMessage(name: string): string {
  switch (name) {
    case "NotAllowedError":
    case "SecurityError":
      return "Microphone access was blocked. Allow wugs to use the microphone in your system's privacy settings, then try again.";
    case "NotFoundError":
    case "OverconstrainedError":
      return "No microphone was found. Plug one in or pick another input.";
    case "NotReadableError":
    case "AbortError":
      return "The microphone could not be opened. Another app may be using it.";
    default:
      return `Recording could not start (${name}).`;
  }
}
