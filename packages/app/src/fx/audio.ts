// Subtle synthesized audio kit — no asset files (so nothing to load or attribute), no React.
// Lazy WebAudio: the AudioContext is created on the FIRST user gesture (`unlock`), so nothing
// ever autoplays; until then every cue is a no-op. Each cue is a short, low-gain enveloped tone —
// "polished and alive", never competing with the calm builder. Mute is persisted to localStorage.
// Framework-free singleton so both Game (imperative) and React chrome call the same instance.

const MUTE_KEY = "ot:audio-muted";
const MASTER = 0.22; // master gain ceiling — deliberately gentle

type Tone = {
  freq: number;
  dur: number; // seconds
  type?: OscillatorType;
  gain?: number; // peak gain before master
  delay?: number; // seconds from trigger
  glideTo?: number; // optional pitch glide target
};

class AudioKit {
  private ctx: AudioContext | null = null;
  private master: GainNode | null = null;
  private _muted = false;

  constructor() {
    try {
      this._muted = localStorage.getItem(MUTE_KEY) === "1";
    } catch {
      /* storage blocked (private mode) — default unmuted */
    }
  }

  get muted(): boolean {
    return this._muted;
  }

  setMuted(m: boolean): void {
    this._muted = m;
    try {
      localStorage.setItem(MUTE_KEY, m ? "1" : "0");
    } catch {
      /* ignore */
    }
    if (this.master && this.ctx) {
      this.master.gain.setTargetAtTime(m ? 0 : MASTER, this.ctx.currentTime, 0.02);
    }
  }

  /** Create/resume the AudioContext. MUST be called from a user gesture (pointer/keydown) — a
   *  no-op once unlocked. Browsers block audio until this happens, so it's wired to first input. */
  unlock(): void {
    if (this.ctx) {
      if (this.ctx.state === "suspended") void this.ctx.resume();
      return;
    }
    const AC: typeof AudioContext | undefined =
      window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AC) return;
    this.ctx = new AC();
    this.master = this.ctx.createGain();
    this.master.gain.value = this._muted ? 0 : MASTER;
    this.master.connect(this.ctx.destination);
  }

  /** Synthesize a short stack of enveloped tones. Silent until unlocked / when muted. */
  private play(tones: Tone[]): void {
    const ctx = this.ctx;
    const master = this.master;
    if (!ctx || !master || this._muted) return;
    const t0 = ctx.currentTime;
    for (const tn of tones) {
      const osc = ctx.createOscillator();
      const g = ctx.createGain();
      osc.type = tn.type ?? "sine";
      const start = t0 + (tn.delay ?? 0);
      const end = start + tn.dur;
      osc.frequency.setValueAtTime(tn.freq, start);
      if (tn.glideTo) osc.frequency.exponentialRampToValueAtTime(tn.glideTo, end);
      // Soft attack, exponential decay (exp ramps can't hit 0 — floor at a hair above).
      const peak = tn.gain ?? 0.18;
      g.gain.setValueAtTime(0.0001, start);
      g.gain.exponentialRampToValueAtTime(peak, start + Math.min(0.02, tn.dur * 0.3));
      g.gain.exponentialRampToValueAtTime(0.0001, end);
      osc.connect(g);
      g.connect(master);
      osc.start(start);
      osc.stop(end + 0.03);
    }
  }

  // --- cues (subtle & satisfying) -------------------------------------------------------------
  /** Placing a station — a soft, short pluck. */
  place(): void {
    this.play([{ freq: 320, dur: 0.07, type: "triangle", gain: 0.15 }]);
  }
  /** A line committed — a gentle rising two-note "connected". */
  connect(): void {
    this.play([
      { freq: 523.25, dur: 0.12, type: "sine", gain: 0.15 }, // C5
      { freq: 783.99, dur: 0.2, type: "sine", gain: 0.16, delay: 0.085 }, // G5
    ]);
  }
  /** A rejected/blocked action — a soft descending "no". */
  alert(): void {
    this.play([
      { freq: 392, dur: 0.13, type: "sine", gain: 0.13 },
      { freq: 277.18, dur: 0.18, type: "sine", gain: 0.13, delay: 0.1 },
    ]);
  }
  /** Tool / mode select — a faint high blip. */
  tick(): void {
    this.play([{ freq: 660, dur: 0.035, type: "sine", gain: 0.07 }]);
  }
  /** Build↔Run flip — a short confident swell. */
  toggle(running: boolean): void {
    this.play([{ freq: running ? 440 : 392, dur: 0.1, type: "triangle", gain: 0.11, glideTo: running ? 587.33 : 329.63 }]);
  }
  /** A milestone crossed (rider/coverage record, "you beat the real network") — a bright rising
   *  arpeggio, distinctly more triumphant than `connect` so the achievement reads as an achievement.
   *  Caller rate-limits (milestones are rare); keep it gentle so it celebrates, never blares. */
  milestone(): void {
    this.play([
      { freq: 523.25, dur: 0.12, type: "triangle", gain: 0.13 }, // C5
      { freq: 659.25, dur: 0.12, type: "triangle", gain: 0.13, delay: 0.08 }, // E5
      { freq: 783.99, dur: 0.14, type: "triangle", gain: 0.14, delay: 0.16 }, // G5
      { freq: 1046.5, dur: 0.22, type: "sine", gain: 0.13, delay: 0.24 }, // C6 — the lift
    ]);
  }
  /** A new day rolls over — a soft low two-note chime (a page turning), under the day report card. */
  day(): void {
    this.play([
      { freq: 293.66, dur: 0.2, type: "sine", gain: 0.1 }, // D4
      { freq: 440, dur: 0.26, type: "sine", gain: 0.1, delay: 0.12 }, // A4
    ]);
  }
  /** A town conquered (fantasy) — a low, brief triumphant swell with a hair of weight (the drum of
   *  the legion arriving). Distinct from `connect`; fired on the conquest beat, not per tick. */
  conquer(): void {
    this.play([
      { freq: 110, dur: 0.22, type: "sawtooth", gain: 0.09 }, // A2 — the weight
      { freq: 196, dur: 0.16, type: "triangle", gain: 0.12, glideTo: 261.63 }, // G3 → C4 lift
      { freq: 392, dur: 0.24, type: "sine", gain: 0.11, delay: 0.1 }, // G4 — the flourish
    ]);
  }
}

export const audio = new AudioKit();
