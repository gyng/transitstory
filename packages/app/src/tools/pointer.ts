// Pointer wiring: translates MapLibre clicks/moves into Game actions. The two build tools
// (place-station, draw-line) and select all funnel through Game methods (Command path).
// Line drawing disables dragPan during the gesture (re-enabled on commit/cancel) — T11.
//
// Build controls (informed by NIMBY/CS/OpenTTD/CAD conventions):
//  • Station tool is STICKY: every click places a station until Esc / right-click / another tool
//    disarms it — so "place 2 stations" (the onboarding's literal step ①) is two clicks, no
//    modifier to discover. (It used to disarm after one placement unless Ctrl/Shift was held.)
//  • Line tool: click stations to chain a route (rubber-band ghost follows the cursor);
//    double-click / Enter commits; Backspace undoes the last waypoint. The station a click
//    would snap to is highlighted BEFORE the click (game.snapStation → white ring).
//  • Esc and right-click both "stop" — a two-stage cancel: drop the in-progress route, then
//    (if nothing pending) leave the build tool (#4). The map pans on LEFT-drag, so right-click
//    is free to use as cancel (it isn't camera-pan here).
import { isDrawTool, type Game } from "../game";
import { DETAIL_ZOOM } from "../config";

export function attachPointer(game: Game): void {
  const map = game.map;
  // Drag state for the line tool: 'handle' = bending a control point, 'draw' = chaining stations
  // by dragging. suppressClick eats the click that fires after a drag press (so it never re-adds).
  let dragging: "handle" | "draw" | null = null;
  let suppressClick = false;

  // Draw-tool press (Track or Service — same gesture): grab/create a control point on the ghost, else
  // begin a drag-to-draw chain.
  map.on("mousedown", (e) => {
    if (game.mode !== "build" || !isDrawTool(game.tool)) return;
    const px = e.point.x;
    const py = e.point.y;
    // Control point on the in-progress ghost (only once there's a span to bend).
    if (game.draft.length >= 2 && game.startHandleDrag(px, py, e.lngLat.lng, e.lngLat.lat)) {
      dragging = "handle";
      suppressClick = true;
      e.preventDefault(); // don't pan
      return;
    }
    // Otherwise pressing a station starts a route. Pressing a TERMINUS of the SELECTED line
    // (Mini-Metro's grab-the-end gesture) extends that line instead — the ghost takes the
    // line's colour so the mode is unmistakable; Esc backs out. Any other station starts a
    // fresh draft as before.
    const id = game.nearestStation(px, py);
    if (id !== null) {
      // Terminus-grab (Mini-Metro extend) is a TRACK-tool gesture only: the Service tool always starts a
      // FRESH service draft, so grabbing a terminus never silently downgrades it to a stockless track-extend.
      if (game.draft.length === 0 && game.selectedLine !== null && game.tool === "line") {
        const lv = game.bridge.linesView()[game.selectedLine];
        if (lv && !lv.removed && !lv.loopLine && lv.stops.length >= 2) {
          const head = lv.stops[0] === id;
          const tail = lv.stops[lv.stops.length - 1] === id;
          if (head || tail) {
            game.startExtend(game.selectedLine, head);
            dragging = "draw";
            suppressClick = true;
            e.preventDefault();
            return;
          }
        }
      }
      game.extendDraft(id);
      dragging = "draw";
      suppressClick = true;
      e.preventDefault();
    }
  });

  map.on("mouseup", () => {
    if (dragging === "handle") game.endHandleDrag();
    dragging = null;
  });

  map.on("click", (e) => {
    if (suppressClick) {
      suppressClick = false;
      return; // this click trailed a drag press — the mousedown already acted
    }
    // A left-click anywhere dismisses an open context menu (it doesn't also act).
    if (game.contextMenu) {
      game.closeContextMenu();
      return;
    }
    const px = e.point.x;
    const py = e.point.y;

    if (game.mode === "build" && game.tool === "station") {
      // Two-stage "confirm build": the click drops a GHOST at the snapped hex cell; the confirm bar
      // (or Enter) commits it, Esc cancels. One-per-cell is checked up front (a taken hex is refused).
      game.ghostStation(e.lngLat.lng, e.lngLat.lat);
      return;
    }

    if (game.mode === "build" && game.tool === "barracks") {
      game.placeBarracks(e.lngLat.lng, e.lngLat.lat); // fantasy: a node that fields legions. Sticky.
      return;
    }

    if (game.mode === "build" && game.tool === "bounty") {
      const town = game.nearestStation(px, py); // fantasy: bait legions toward the nearest town. Sticky.
      if (town !== null) game.postBounty(town);
      return;
    }

    if (game.mode === "build" && isDrawTool(game.tool)) {
      // Station chaining + control points are handled on mousedown/drag now; nothing to do here
      // (and crucially, don't fall through to Select and clear the draft). Track + Service alike.
      return;
    }

    if (game.mode === "build" && game.tool === "bulldozer") {
      game.bulldozeAt(px, py); // remove nearest station, else nearest line — stays armed
      return;
    }

    // Select tool (or run mode): pin the nearest station; else (running, with peeps revealed)
    // "pick up" the nearest rider under the cursor → the FollowCard opens on them; else deselect.
    // Stations stay the higher-priority, larger target — peep-pick never steals a station selection.
    const hit = game.nearestStation(px, py);
    if (hit !== null) { game.selectStation(hit); return; }
    if (game.mode === "run" && game.showPeeps && map.getZoom() >= DETAIL_ZOOM && game.inspectPeepAt(px, py)) {
      return; // followed a rider (inspectPeepAt opened the FollowCard); nothing more to do
    }
    // TTD L6: a click near a LINE selects it (so clicking the grey bare track opens its editor to assign
    // stock — the on-screen copy promises this). Stations win (handled above); empty space deselects.
    const ln = game.nearestLine(px, py);
    if (ln !== null) game.selectLine(ln);
    else game.clearSelection();
  });

  // Double-click a control point removes it (straighten); otherwise commit the draft. Enter also
  // commits; Escape / right-click cancel (T11).
  map.on("dblclick", (e) => {
    if (isDrawTool(game.tool) && game.draft.length >= 2) {
      e.preventDefault();
      if (game.removeHandleAt(e.point.x, e.point.y)) return; // straighten this bend, keep drawing
      game.commitDraft();
    }
  });

  // Right-click. In BUILD it stays the two-stage "stop" (drop the draft, then leave the tool) — this
  // branch is verbatim so draft-cancel is mechanically preserved. In run/select it opens the context
  // menu at the cursor (resolved station → line → empty). preventDefault suppresses the browser menu;
  // pan is left-drag here, so right-click never fights the camera.
  map.on("contextmenu", (e) => {
    e.preventDefault?.();
    // Right-click ABORTS an in-progress line draft (the genuinely useful cancel). With nothing to abort,
    // it INSPECTS whatever's under the cursor — in any mode — so a cart, rider, station, town or resource
    // is one right-click away (build mode included; left-click still places/draws).
    if (game.mode === "build" && game.draft.length > 0) {
      game.stopBuilding();
      return;
    }
    game.openContextMenu(e.point.x, e.point.y, e.lngLat);
  });
  // Camera moves dismiss an open menu (it would otherwise float over a stale location).
  map.on("movestart", () => game.closeContextMenu());

  map.on("mousemove", (e) => {
    // Pre-commit snap highlight (AGENTS UX: "highlight the snap candidate BEFORE the click
    // commits"): in the line tool, the station a click/drag would chain; in the bulldozer, the
    // station a click would demolish. Cleared for every other tool.
    const snappable = game.mode === "build" && (isDrawTool(game.tool) || game.tool === "bulldozer");
    const snap = snappable ? game.nearestStation(e.point.x, e.point.y) : null;
    const snapChanged = snap !== game.snapStation;
    game.snapStation = snap;

    // Bending a control point: move it under the cursor (sub-100 ms, client-side).
    if (dragging === "handle") {
      game.dragHandle(e.lngLat.lng, e.lngLat.lat);
      return;
    }
    // Drag-to-draw: chain any station the cursor reaches, rubber-banding to the cursor.
    if (dragging === "draw") {
      if (snap !== null) game.extendDraft(snap);
      game.cursor = [e.lngLat.lng, e.lngLat.lat];
      game.refresh();
      return;
    }
    // Live blueprint cursor while drawing (Track or Service).
    if (isDrawTool(game.tool) && game.draft.length >= 1) {
      game.cursor = [e.lngLat.lng, e.lngLat.lat];
      game.refresh();
      return;
    }
    // Hover highlight: show the nearest station's catchment.
    const id = game.nearestStation(e.point.x, e.point.y);
    if (id !== game.hoveredStation || snapChanged) {
      game.hoveredStation = id;
      game.refresh();
    }
  });

  window.addEventListener("keydown", (e) => {
    const tag = (e.target as HTMLElement | null)?.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
    if (game.pendingStation && e.key === "Enter") {
      game.confirmPendingStation(); // commit the ghost station
    } else if (game.pendingStation && e.key === "Escape") {
      game.cancelPendingStation(); // discard the ghost
    } else if (e.key === "Enter" && isDrawTool(game.tool) && game.draft.length >= 2) {
      game.commitDraft();
    } else if (e.key === "Backspace" && isDrawTool(game.tool) && game.draft.length > 0) {
      e.preventDefault(); // don't let the browser navigate back
      game.popDraft();
    } else if (e.key === "Escape") {
      game.stopBuilding();
    }
  });
}
