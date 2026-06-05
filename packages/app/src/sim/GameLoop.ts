// Fixed-timestep accumulator (Gaffer "Fix Your Timestep"): step the sim at a constant
// TICK_MS while Running, and each animation frame interpolate vehicle positions between the
// previous and current sim snapshot by alpha = accumulator/dt. Render (rAF + wall-clock)
// stays decoupled from the deterministic sim. Stats DOM lives on its own ~3 Hz throttle.
import { TICK_MS } from "../config";
import type { Game } from "../game";

export class GameLoop {
  private acc = 0;
  private last = 0;
  private raf = 0;
  private speed = 1;

  constructor(readonly game: Game) {}

  start(): void {
    this.last = performance.now();
    this.raf = requestAnimationFrame(this.frame);
  }

  stop(): void {
    cancelAnimationFrame(this.raf);
  }

  /** Sim-speed multiplier (1×/10×/max) — scales how many fixed steps run per real second. */
  setSpeed(mult: number): void {
    this.speed = mult;
  }

  private frame = (now: number): void => {
    let dt = now - this.last;
    this.last = now;
    if (dt > 250) dt = 250; // clamp after a tab switch to avoid a step spiral

    let alpha = 0;
    if (this.game.mode === "run") {
      this.acc += dt * this.speed;
      let steps = 0;
      while (this.acc >= TICK_MS && steps < 10_000) {
        this.game.bridge.tick(TICK_MS);
        this.acc -= TICK_MS;
        steps++;
      }
      alpha = this.acc / TICK_MS;
    }

    this.renderVehicles(alpha);
    this.raf = requestAnimationFrame(this.frame);
  };

  private renderVehicles(alpha: number): void {
    // Interpolation + dot assembly (line tint, heading, load) lives in Game.vehicleDotsAt so the
    // per-frame loop and the on-refresh recompose share one source. composeAndSet handles empty.
    this.game.composeAndSet(this.game.vehicleDotsAt(alpha));
  }
}
