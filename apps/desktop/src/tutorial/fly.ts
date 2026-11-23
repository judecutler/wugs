// The Tutorial button's jump into the Project menu: a copy of the button
// flies along the same arc as the wug's jump (`hopFrames`) and the menu
// button gives a small bounce when it lands. Skipped under reduced motion.
import { hopFrames } from "../help/logic";

const FLIGHT_MS = 650;

/** `from` is where the button was; `to` is looked up after the top bar redraws. */
export function flyInto(copy: HTMLElement, from: DOMRect, to: () => Element | null): void {
  const target = to();
  const b = target?.getBoundingClientRect();
  if (b === undefined || from.width === 0 || b.width === 0 || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  copy.classList.add("tutorial-flyer");
  copy.setAttribute("aria-hidden", "true");
  copy.tabIndex = -1;
  Object.assign(copy.style, { left: "0px", top: "0px", width: `${from.width}px`, height: `${from.height}px` });
  document.body.append(copy);
  const frames = hopFrames({ x: from.left, y: from.top, width: from.width, height: from.height }, { x: b.left + (b.width - from.width * 0.5) / 2, y: b.top, width: from.width * 0.5, height: b.height }, 16, 50);
  const anim = copy.animate(
    frames.map((f) => ({ offset: f.t, opacity: f.t < 0.85 ? 1 : (1 - f.t) / 0.15, transform: `translate(${f.x}px, ${f.y}px) scale(${f.size / from.width}) rotate(${Math.sin(f.t * Math.PI) * 8}deg)` })),
    { duration: FLIGHT_MS, easing: "linear" },
  );
  const done = () => {
    copy.remove();
    const landed = to();
    landed?.classList.add("menu-landed");
    window.setTimeout(() => landed?.classList.remove("menu-landed"), 450);
  };
  anim.onfinish = done;
  anim.oncancel = done;
}
