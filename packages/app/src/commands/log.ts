// In-memory command log: the save artifact (seed + log) and the undo/replay/multiplayer
// seam. The frontend never mutates sim state directly; it appends here as it sends commands.
import type { Command } from "../types";

export class CommandLog {
  private cmds: Command[] = [];

  push(c: Command): void {
    this.cmds.push(c);
  }

  all(): readonly Command[] {
    return this.cmds;
  }

  get length(): number {
    return this.cmds.length;
  }

  /** Drop the most recent command (basis for undo = rebuild from seed + log[..-1]). */
  popLast(): Command | undefined {
    return this.cmds.pop();
  }

  /** Replace the whole log (loading a save). The caller rebuilds the Sim from it. */
  replace(cmds: readonly Command[]): void {
    this.cmds = [...cmds];
  }
}
