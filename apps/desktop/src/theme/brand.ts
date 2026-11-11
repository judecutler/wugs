import { h } from "../dom";

/** Where the owner's logo goes (public/brand/README.md). */
export const LOGO_SRC = "/brand/logo.svg";

let missing = false;

/** The suite logo for the top bar, or null once it is known not to exist,
 * so a build without the asset shows the kit name alone and never a broken
 * image. */
export function brandLogo(): HTMLImageElement | null {
  if (missing) return null;
  const img = h("img", { className: "brand-logo", src: LOGO_SRC, alt: "wugs" });
  img.onerror = () => {
    missing = true;
    img.remove();
  };
  return img;
}
