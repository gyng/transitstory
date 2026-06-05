import { test, expect } from "@playwright/test";

// Freeform waypoints (control points that bend the track) — driven camera-independently through
// Game (extendDraft / addDraftWaypoint route through coords/geo.ts, the production boundary).
// Asserts the GAMEPLAY FACT: after bending a span and committing, the authoritative line geometry
// genuinely curves off the straight chord, and the waypoint never became a stop.
test("a control point bends the committed line's track without adding a stop", async ({ page }) => {
  await page.goto("/?city=singapore");
  await page.waitForFunction(() => window.__APP_READY === true, undefined, { timeout: 30_000 });

  // Two stations ~2 km apart along the same latitude (a horizontal chord).
  const ids = await page.evaluate(() => {
    const t = window.__ot_test!;
    return [t.placeStationLngLat(103.84, 1.29), t.placeStationLngLat(103.86, 1.29)] as unknown as number[];
  });
  expect(await page.evaluate(() => window.__ot!.bridge.stationsView().length)).toBe(2);

  // Build a 2-stop draft (one span), then bend that span ~2 km NORTH of the chord midpoint — the
  // camera-independent equivalent of dragging the span's "+" handle.
  const handles = await page.evaluate((sids) => {
    const g = window.__ot!.game;
    g.setMode("build");
    g.setTool("line");
    g.setTransport(0);
    g.extendDraft((sids as number[])[0]);
    g.extendDraft((sids as number[])[1]);
    const before = g.controlHandles().map((h: { kind: string }) => h.kind);
    g.addDraftWaypoint(0, 103.85, 1.31); // bend span 0 north
    const after = g.controlHandles().map((h: { kind: string }) => h.kind);
    return { before, after, spans: g.draftWaypoints.length };
  }, ids);
  // Before bending: just the "+" midpoint. After: a draggable waypoint + two fresh "+" midpoints.
  expect(handles.before).toEqual(["add"]);
  expect(handles.after.filter((k) => k === "waypoint")).toHaveLength(1);
  expect(handles.spans).toBe(1);

  // Commit, then measure how far the committed polyline bows off the straight A→B chord.
  const result = await page.evaluate(() => {
    const g = window.__ot!.game;
    const before = window.__ot!.bridge.linesView().length;
    g.commitDraft();
    const lines = window.__ot!.bridge.linesView();
    const line = lines[lines.length - 1];
    const poly = line.polylineMm as [number, number][];
    const p0 = poly[0];
    const p1 = poly[poly.length - 1];
    const dx = p1[0] - p0[0];
    const dy = p1[1] - p0[1];
    const len = Math.hypot(dx, dy) || 1;
    let maxOff = 0;
    for (const [x, y] of poly) {
      const off = Math.abs((x - p0[0]) * dy - (y - p0[1]) * dx) / len;
      if (off > maxOff) maxOff = off;
    }
    return { created: lines.length - before, stops: line.stops.length, maxOffMm: maxOff };
  });

  expect(result.created).toBe(1); // exactly one line committed
  expect(result.stops).toBe(2); // the waypoint shaped the curve but is NOT a halt
  expect(result.maxOffMm).toBeGreaterThan(500_000); // the track genuinely bends (>0.5 km off the chord)

  await page.screenshot({ path: "../../docs/progress/waypoints-bent-line.png" });
});
