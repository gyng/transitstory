// Pointer wiring: translates MapLibre clicks/moves into Game actions. The two build tools
// (place-station, draw-line) and select all funnel through Game methods (Command path).
// Line drawing disables dragPan during the gesture (re-enabled on commit/cancel) — T11.
import type { Game } from "../game";

export function attachPointer(game: Game): void {
  const map = game.map;

  map.on("click", (e) => {
    const px = e.point.x;
    const py = e.point.y;

    if (game.mode === "build" && game.tool === "station") {
      game.placeStation(e.lngLat.lng, e.lngLat.lat);
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

  // Double-click / Enter commits a line draft; Escape cancels (T11).
  map.on("dblclick", (e) => {
    if (game.tool === "line" && game.draft.length >= 2) {
      e.preventDefault();
      game.commitDraft();
    }
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
    if (e.key === "Enter" && game.tool === "line" && game.draft.length >= 2) game.commitDraft();
    if (e.key === "Escape") game.cancelDraft(), game.refresh();
  });
}
