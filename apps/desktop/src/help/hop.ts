// The wug's jump from the top bar into the Help panel (roadmap 67): a copy
// of the top bar's wug flies along `hopFrames` and lands on the panel's wug,
// which is hidden until it arrives. Skipped under reduced motion.
import { currentWug, wugSvg } from "../theme/wug";
import { hopFrames } from "./logic";

const FLIGHT_MS = 620;
let flying: Animation | null = null;

/** `logo` is looked up when the jump starts: opening the panel redraws the top bar. */
export function hopInto(logo: () => Element | null, to: HTMLElement): void {
  const from = logo();
  const a = from?.getBoundingClientRect();
  const b = to.getBoundingClientRect();
  if (a === undefined || a.width === 0 || b.width === 0 || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  flying?.cancel();
  const flyer = document.createElement("span");
  flyer.className = "wug-hop";
  flyer.setAttribute("aria-hidden", "true");
  flyer.innerHTML = wugSvg(currentWug(), "");
  Object.assign(flyer.style, { left: "0px", top: "0px", width: `${a.width}px`, height: `${a.width}px` });
  document.body.append(flyer);
  from?.classList.add("wug-away");
  to.classList.add("wug-landing");
  const frames = hopFrames({ x: a.left, y: a.top, width: a.width, height: a.height }, { x: b.left, y: b.top, width: b.width, height: b.height }, 16);
  const keyframes: Keyframe[] = frames.map((f) => ({
    offset: f.t,
    transform: `translate(${f.x}px, ${f.y}px) scale(${f.size / a.width}) rotate(${Math.sin(f.t * Math.PI) * -14}deg)`,
  }));
  const anim = flyer.animate(keyframes, { duration: FLIGHT_MS, easing: "linear" });
  flying = anim;
  const done = () => {
    flyer.remove();
    to.classList.remove("wug-landing");
    to.classList.add("wug-landed");
    window.setTimeout(() => to.classList.remove("wug-landed"), 400);
    window.setTimeout(() => logo()?.classList.remove("wug-away"), 900);
    if (flying === anim) flying = null;
  };
  anim.onfinish = done;
  anim.oncancel = done;
}
