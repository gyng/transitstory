// Spatial juice on a DEDICATED canvas — the sanctioned "canvas tween", NOT a deck-layer rebuild
// and NOT a sim tick. A transparent canvas inside the map container (z between the map/deck and the
// #ui chrome, pointer-events:none), redrawn each frame from the EXISTING GameLoop rAF. Effects
// anchor to lng/lat and are projected via map.project every frame, so they track pan/zoom for free.
// Feel: "subtle & satisfying" — soft eased rings/flashes/bursts that fade then cull. Decoupled from
// the deterministic core: purely client-side acknowledgement, exactly like the blueprint preview.
import type { Map as MlMap } from "maplibre-gl";

type Ring = {
  lng: number;
  lat: number;
  born: number; // performance.now() ms
  ttl: number; // ms
  rgb: string; // "r,g,b"
  r0: number; // start radius (px)
  r1: number; // end radius (px)
  w0: number; // start line width (px)
  alpha: number; // peak alpha
};

type Flash = {
  pts: [number, number][]; // lng/lat polyline of the committed line
  born: number;
  ttl: number;
  rgb: string;
};

type Throb = { lng: number; lat: number; rgb: string };

const easeOut = (t: number): number => 1 - (1 - t) * (1 - t) * (1 - t); // cubic ease-out

export class Effects {
  private canvas: HTMLCanvasElement;
  private cx: CanvasRenderingContext2D;
  private rings: Ring[] = [];
  private flashes: Flash[] = [];
  private throbs: Throb[] = []; // continuous (starved stations) — replaced wholesale per stats tick
  private dpr = 1;
  private wasActive = false; // so an idle canvas is a true no-op (no per-frame clear when nothing's live)

  constructor(private map: MlMap) {
    this.canvas = document.createElement("canvas");
    this.canvas.setAttribute("data-ot-fx", "");
    Object.assign(this.canvas.style, {
      position: "absolute",
      inset: "0",
      width: "100%",
      height: "100%",
      pointerEvents: "none",
      zIndex: "4", // above the map+deck (#map stacking) and below the #ui chrome (z:5)
    } satisfies Partial<CSSStyleDeclaration>);
    map.getContainer().appendChild(this.canvas);
    this.cx = this.canvas.getContext("2d")!;
    this.resize();
    map.on("resize", this.resize);
  }

  private resize = (): void => {
    const el = this.map.getContainer();
    this.dpr = Math.min(2, window.devicePixelRatio || 1);
    this.canvas.width = Math.max(1, Math.round(el.clientWidth * this.dpr));
    this.canvas.height = Math.max(1, Math.round(el.clientHeight * this.dpr));
  };

  // --- emitters (called from Game on a command's client echo) --------------------------------

  /** Expanding ring at a point — placement acknowledgement / generic ping. */
  ripple(lng: number, lat: number, rgb = "0,114,178"): void {
    this.rings.push({ lng, lat, born: performance.now(), ttl: 620, rgb, r0: 4, r1: 34, w0: 3, alpha: 0.55 });
  }

  /** Soft selection pulse — a single gentle ring (fainter, larger) when a station is pinned. */
  pulse(lng: number, lat: number, rgb = "0,114,178"): void {
    this.rings.push({ lng, lat, born: performance.now(), ttl: 520, rgb, r0: 8, r1: 30, w0: 2.5, alpha: 0.4 });
  }

  /** Small activity burst — a quick faint ring where riders just boarded (3 Hz delta-driven). */
  burst(lng: number, lat: number, rgb = "0,158,115"): void {
    this.rings.push({ lng, lat, born: performance.now(), ttl: 460, rgb, r0: 3, r1: 16, w0: 2, alpha: 0.5 });
  }

  /** Connect flash — the newly committed line lights up: a bright glide down its length + glow. */
  connectFlash(pts: [number, number][], rgb = "255,255,255"): void {
    if (pts.length < 2) return;
    this.flashes.push({ pts, born: performance.now(), ttl: 720, rgb });
  }

  /** Replace the continuously-throbbing set (e.g. starved stations) — called on the stats tick. */
  setThrobs(points: { lng: number; lat: number }[], rgb = "214,40,40"): void {
    this.throbs = points.map((p) => ({ lng: p.lng, lat: p.lat, rgb }));
  }

  clear(): void {
    this.rings = [];
    this.flashes = [];
    this.throbs = [];
  }

  // --- per-frame draw (called from GameLoop.frame with the rAF timestamp) ---------------------

  draw(now: number): void {
    const cx = this.cx;
    // True idle no-op: when nothing is live AND nothing was live last frame, do zero work (not even
    // a clear). Only when we just went idle do we clear the last frame's pixels once.
    if (this.rings.length === 0 && this.flashes.length === 0 && this.throbs.length === 0) {
      if (this.wasActive) {
        cx.clearRect(0, 0, this.canvas.width, this.canvas.height);
        this.wasActive = false;
      }
      return;
    }
    this.wasActive = true;
    cx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    cx.clearRect(0, 0, this.canvas.width / this.dpr, this.canvas.height / this.dpr);

    this.drawThrobs(now);
    this.drawRings(now);
    this.drawFlashes(now);
  }

  private drawThrobs(now: number): void {
    if (this.throbs.length === 0) return;
    const cx = this.cx;
    // Gentle shared sine breath (~1.4 s) — a living "fix me" without a per-frame layer rebuild.
    const s = 0.5 + 0.5 * Math.sin((now / 1400) * Math.PI * 2);
    const r = 11 + s * 5;
    const a = 0.22 + s * 0.33;
    for (const th of this.throbs) {
      const p = this.map.project([th.lng, th.lat]);
      cx.beginPath();
      cx.arc(p.x, p.y, r, 0, Math.PI * 2);
      cx.strokeStyle = `rgba(${th.rgb},${a.toFixed(3)})`;
      cx.lineWidth = 2.5;
      cx.stroke();
    }
  }

  private drawRings(now: number): void {
    const cx = this.cx;
    const live: Ring[] = [];
    for (const e of this.rings) {
      const t = (now - e.born) / e.ttl;
      if (t >= 1) continue;
      live.push(e);
      const k = easeOut(t);
      const r = e.r0 + (e.r1 - e.r0) * k;
      const p = this.map.project([e.lng, e.lat]);
      cx.beginPath();
      cx.arc(p.x, p.y, r, 0, Math.PI * 2);
      cx.strokeStyle = `rgba(${e.rgb},${(e.alpha * (1 - t)).toFixed(3)})`;
      cx.lineWidth = Math.max(0.5, e.w0 * (1 - t));
      cx.stroke();
    }
    this.rings = live;
  }

  private drawFlashes(now: number): void {
    const cx = this.cx;
    const live: Flash[] = [];
    for (const f of this.flashes) {
      const t = (now - f.born) / f.ttl;
      if (t >= 1) continue;
      live.push(f);
      const scr = f.pts.map((ll) => this.map.project(ll));
      // (a) a soft full-line glow that fades over the whole flash.
      cx.beginPath();
      cx.moveTo(scr[0].x, scr[0].y);
      for (let i = 1; i < scr.length; i++) cx.lineTo(scr[i].x, scr[i].y);
      cx.strokeStyle = `rgba(${f.rgb},${(0.6 * (1 - t)).toFixed(3)})`;
      cx.lineWidth = 7 * (1 - t) + 1;
      cx.lineJoin = "round";
      cx.lineCap = "round";
      cx.stroke();
      // (b) a bright head gliding from start → end (energy flowing into the new line).
      const head = this.pointAtFrac(scr, easeOut(Math.min(1, t * 1.15)));
      if (head) {
        const grad = cx.createRadialGradient(head.x, head.y, 0, head.x, head.y, 9);
        grad.addColorStop(0, `rgba(255,255,255,${(0.9 * (1 - t)).toFixed(3)})`);
        grad.addColorStop(1, "rgba(255,255,255,0)");
        cx.fillStyle = grad;
        cx.beginPath();
        cx.arc(head.x, head.y, 9, 0, Math.PI * 2);
        cx.fill();
      }
    }
    this.flashes = live;
  }

  /** Point at arc-length fraction `f` (0..1) along a projected polyline. */
  private pointAtFrac(scr: { x: number; y: number }[], f: number): { x: number; y: number } | null {
    if (scr.length < 2) return null;
    let total = 0;
    const seg: number[] = [];
    for (let i = 1; i < scr.length; i++) {
      const d = Math.hypot(scr[i].x - scr[i - 1].x, scr[i].y - scr[i - 1].y);
      seg.push(d);
      total += d;
    }
    if (total === 0) return scr[0];
    let target = f * total;
    for (let i = 0; i < seg.length; i++) {
      if (target <= seg[i]) {
        const r = seg[i] === 0 ? 0 : target / seg[i];
        return { x: scr[i].x + (scr[i + 1].x - scr[i].x) * r, y: scr[i].y + (scr[i + 1].y - scr[i].y) * r };
      }
      target -= seg[i];
    }
    return scr[scr.length - 1];
  }
}
