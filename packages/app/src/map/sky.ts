// Day/night mood wash over the basemap. Two NON-interactive divs appended into the map container,
// above both the MapLibre and deck.gl canvases:
//   • #ot-sky      (mix-blend-mode: multiply) — darkens the near-white Positron ground toward night.
//     multiply can only pull the bright ground DOWN toward where the dark-cased network already
//     lives, so it never veils the saturated line ribbons (a vivid colour × a dim desaturated tint
//     keeps >80% luminance + full hue) — readability is preserved, midday is a true no-op.
//   • #ot-sky-glow (mix-blend-mode: screen)   — adds warm dawn/dusk horizon light.
// Driven by sim HOUR off the existing ~3 Hz stats slice (two-clocks: never per rAF/tick). An .8s CSS
// crossfade makes band changes glide. The tint carries NO categorical meaning (lightness/temperature
// only) so it's colour-blind-safe and never competes with the Okabe-Ito line hues.

type RGBA = [number, number, number, number];
interface Stop {
  h: number;
  mult: RGBA;
  glow: RGBA;
}

// Hour anchors (lerped between the two bracketing stops each tick). Night is the MOST tinted
// (multiply ≤ 0.22), dusk the warmest (glow ≤ 0.14), midday a genuine 0/0 readability baseline.
const STOPS: Stop[] = [
  { h: 0, mult: [36, 46, 78, 0.22], glow: [0, 0, 0, 0] }, // deep night — desaturated indigo
  { h: 5, mult: [48, 54, 86, 0.18], glow: [255, 150, 90, 0.05] }, // pre-dawn
  { h: 7, mult: [120, 112, 124, 0.06], glow: [255, 176, 102, 0.1] }, // dawn — warm amber light
  { h: 10, mult: [165, 175, 195, 0.03], glow: [0, 0, 0, 0] }, // morning — barely cool/crisp
  { h: 13, mult: [0, 0, 0, 0], glow: [0, 0, 0, 0] }, // MIDDAY — pristine Positron, the baseline
  { h: 16, mult: [180, 168, 150, 0.035], glow: [0, 0, 0, 0] }, // afternoon — faint warmth
  { h: 17.5, mult: [72, 60, 94, 0.13], glow: [255, 142, 80, 0.14] }, // dusk — violet ground + amber horizon
  { h: 19.5, mult: [46, 54, 88, 0.18], glow: [255, 150, 96, 0.05] }, // blue hour
  { h: 22, mult: [36, 46, 78, 0.22], glow: [0, 0, 0, 0] }, // night
  { h: 24, mult: [36, 46, 78, 0.22], glow: [0, 0, 0, 0] }, // wrap == h:0
];

const mix = (a: number, b: number, t: number) => a + (b - a) * t;
const mixRGBA = (a: RGBA, b: RGBA, t: number): RGBA => [mix(a[0], b[0], t), mix(a[1], b[1], t), mix(a[2], b[2], t), mix(a[3], b[3], t)];
const css = (c: RGBA) => `rgba(${Math.round(c[0])},${Math.round(c[1])},${Math.round(c[2])},${c[3].toFixed(3)})`;

export interface Sky {
  /** Recolour for the given sim hour (0..24 float). Cheap — call off the ~3 Hz stats slice. */
  set(hour: number): void;
  /** Master toggle (Settings). When off, both layers go transparent (the clinical view). */
  setEnabled(on: boolean): void;
}

export function createSky(container: HTMLElement): Sky {
  const make = (id: string, blend: string): HTMLDivElement => {
    const d = document.createElement("div");
    d.id = id;
    Object.assign(d.style, {
      position: "absolute",
      inset: "0",
      pointerEvents: "none",
      mixBlendMode: blend,
      transition: "background-color .8s linear",
      background: "transparent",
      zIndex: "2", // above the basemap + deck canvases (which sit at the default z)
    } as CSSStyleDeclaration);
    container.appendChild(d);
    return d;
  };
  const sky = make("ot-sky", "multiply");
  const glow = make("ot-sky-glow", "screen");
  let enabled = true;

  function set(hour: number): void {
    if (!enabled) {
      sky.style.background = "transparent";
      glow.style.background = "transparent";
      return;
    }
    const h = ((hour % 24) + 24) % 24;
    let i = 0;
    while (i < STOPS.length - 1 && STOPS[i + 1].h <= h) i++;
    const a = STOPS[i];
    const b = STOPS[Math.min(i + 1, STOPS.length - 1)];
    const t = b.h > a.h ? (h - a.h) / (b.h - a.h) : 0;
    sky.style.background = css(mixRGBA(a.mult, b.mult, t));
    glow.style.background = css(mixRGBA(a.glow, b.glow, t));
  }

  return {
    set,
    setEnabled(on: boolean) {
      enabled = on;
      if (!on) {
        sky.style.background = "transparent";
        glow.style.background = "transparent";
      }
    },
  };
}
