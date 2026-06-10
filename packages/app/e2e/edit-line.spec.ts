import { test, expect } from "@playwright/test";

// Line editing — extend (both termini), insert mid-line, redo. All inside the existing Command
// vocabulary (AddStop{after} + the replayed log); these assert the committed stop ORDER, not just
// counts, because ordering is the whole point of after-indexed insertion.
test("extend a line from both ends, insert a stop mid-line, undo/redo it", async ({ page }) => {
  await page.goto("/?city=singapore");
  await page.waitForFunction(() => window.__MAP_READY === true, undefined, { timeout: 30_000 });

  // A 2-stop line A→B on known-buildable land (same corridor as the slice spec).
  const ids = await page.evaluate(() => {
    const t = window.__ot_test!;
    const a = t.placeStationLngLat(103.84, 1.281); // CBD
    const b = t.placeStationLngLat(103.826, 1.291); // Tiong Bahru
    const line = t.drawLine([a, b]);
    return { a, b, line };
  });

  // Extend from the TAIL (B): seed draft at B, chain C, commit → A,B,C.
  const c = await page.evaluate(({ line }) => {
    const g = window.__ot!.game;
    const t = window.__ot_test!;
    const c = t.placeStationLngLat(103.832, 1.304); // Orchard
    g.selectLine(line);
    if (!g.startExtend(line, false)) throw new Error("startExtend tail refused");
    g.extendDraft(c);
    g.commitDraft();
    return c;
  }, ids);
  let stops = await page.evaluate(({ line }) => window.__ot!.bridge.linesView()[line].stops, ids);
  expect(stops).toEqual([ids.a, ids.b, c]);

  // Extend from the HEAD (A): chain D outward, commit → D,A,B,C (insert-at-0 preserves the
  // drawn order outward from the old head).
  const d = await page.evaluate(({ line }) => {
    const g = window.__ot!.game;
    const t = window.__ot_test!;
    const d = t.placeStationLngLat(103.847, 1.272); // south-east of the CBD head
    if (!g.startExtend(line, true)) throw new Error("startExtend head refused");
    g.extendDraft(d);
    g.commitDraft();
    return d;
  }, ids);
  stops = await page.evaluate(({ line }) => window.__ot!.bridge.linesView()[line].stops, ids);
  expect(stops).toEqual([d, ids.a, ids.b, c]);

  // Insert mid-line: a station sitting closest to the B→C span joins between them.
  const e = await page.evaluate(({ line }) => {
    const g = window.__ot!.game;
    const t = window.__ot_test!;
    const e = t.placeStationLngLat(103.8285, 1.2975); // between Tiong Bahru and Orchard
    if (!g.insertStopOnLine(line, e)) throw new Error("insertStopOnLine refused");
    return e;
  }, ids);
  stops = await page.evaluate(({ line }) => window.__ot!.bridge.linesView()[line].stops, ids);
  expect(stops).toEqual([d, ids.a, ids.b, e, c]);

  // Undo pops the insertion; redo replays it; a fresh command forks (clears) the redo stack.
  await page.evaluate(() => window.__ot!.game.undo());
  stops = await page.evaluate(({ line }) => window.__ot!.bridge.linesView()[line].stops, ids);
  expect(stops).toEqual([d, ids.a, ids.b, c]);
  expect(await page.evaluate(() => window.__ot!.game.canRedo())).toBe(true);

  await page.evaluate(() => window.__ot!.game.redo());
  stops = await page.evaluate(({ line }) => window.__ot!.bridge.linesView()[line].stops, ids);
  expect(stops).toEqual([d, ids.a, ids.b, e, c]);

  await page.evaluate(() => window.__ot!.game.undo());
  await page.evaluate(() => window.__ot_test!.placeStationLngLat(103.79, 1.345)); // fresh command
  expect(await page.evaluate(() => window.__ot!.game.canRedo())).toBe(false);

  // The vertical fact: the edited line still runs — assign trains, run, riders board.
  await page.evaluate(({ line }) => {
    const t = window.__ot_test!;
    t.assignTrainset(line, 3);
    t.setRunning(true);
    t.setSpeed(100);
  }, ids);
  await page.waitForFunction(
    () => (window.__ot_test!.stats() as { ridershipTotal: number }).ridershipTotal > 0,
    undefined,
    { timeout: 15_000 },
  );
});
