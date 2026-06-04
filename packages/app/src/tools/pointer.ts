// Pointer wiring: translates MapLibre clicks/moves into Game actions. The two build tools
// (place-station, draw-line) and select all funnel through Game methods (Command path).
// Line drawing disables dragPan during the gesture (re-enabled on commit/cancel) — T11.
//
// Build controls (informed by NIMBY/CS/OpenTTD/CAD conventions):
//  • Station tool places ONE station per click, then disarms to Select — UNLESS a modifier
//    (Ctrl / Cmd / Shift) is held, which keeps it armed for rapid placement (#3).
//  • Line tool: click stations to chain a route (rubber-band ghost follows the cursor);
//    double-click / Enter commits; Backspace undoes the last waypoint.
//  • Esc and right-click both "stop" — a two-stage cancel: drop the in-progress route, then
//    (if nothing pending) leave the build tool (#4). The map pans on LEFT-drag, so right-click
//    is free to use as cancel (it isn't camera-pan here).
import type { Game } from "../game";

export function attachPointer(game: Game): void {
  const map = game.map;

  map.on("click", (e) => {
    const px = e.point.x;
    const py = e.point.y;
    const oe = e.originalEvent;
    const keepPlacing = oe.ctrlKey || oe.metaKey || oe.shiftKey;

    if (game.mode === "build" && game.tool === "station") {
      game.placeStation(e.lngLat.lng, e.lngLat.lat);
      // One at a time: disarm to Select after a single placement unless a modifier is held.
      if (!keepPlacing) game.setTool("select");
      return;
    }

    if (game.mode === "build" && game.tool === "line") {
      const id = game.nearestStation(px, py);
      if (id !== null) game.extendDraft(id);
      return;
    }

    // Select tool (or run mode): pick the nearest station.
    game.selectStation(game.nearestStation(px, py));
  });

  // Double-click / Enter commits a line draft; Escape / right-click cancel (T11).
  map.on("dblclick", (e) => {
    if (game.tool === "line" && game.draft.length >= 2) {
      e.preventDefault();
      game.commitDraft();
    }
  });

  // Right-click = stop building/routing (two-stage). preventDefault suppresses the browser menu;
  // pan is left-drag here, so right-click never fights the camera.
  map.on("contextmenu", (e) => {
    if (game.mode !== "build") return;
    e.preventDefault?.();
    game.stopBuilding();
  });

  map.on("mousemove", (e) => {
    // Live blueprint cursor while drawing.
    if (game.tool === "line" && game.draft.length >= 1) {
      game.cursor = [e.lngLat.lng, e.lngLat.lat];
      game.refresh();
      return;
    }
    // Hover highlight: show the nearest station's catchment.
    const id = game.nearestStation(e.point.x, e.point.y);
    if (id !== game.hoveredStation) {
      game.hoveredStation = id;
      game.refresh();
    }
  });

  window.addEventListener("keydown", (e) => {
    const tag = (e.target as HTMLElement | null)?.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
    if (e.key === "Enter" && game.tool === "line" && game.draft.length >= 2) {
      game.commitDraft();
    } else if (e.key === "Backspace" && game.tool === "line" && game.draft.length > 0) {
      e.preventDefault(); // don't let the browser navigate back
      game.popDraft();
    } else if (e.key === "Escape") {
      game.stopBuilding();
    }
  });
}
