// Start menu: pick a city and a start mode (real network vs empty sandbox), then boot.
import { CITIES, type CityEntry } from "../sim/cities";

export function showMenu(onStart: (city: CityEntry, withNetwork: boolean) => void): void {
  const overlay = document.createElement("div");
  overlay.id = "menu";
  overlay.dataset.testid = "menu";
  overlay.style.cssText =
    "position:fixed;inset:0;z-index:50;display:flex;align-items:center;justify-content:center;" +
    "background:radial-gradient(circle at 50% 30%,#2a3138,#11151a);color:#eef1f4;font-family:system-ui,sans-serif";

  const card = document.createElement("div");
  card.style.cssText = "width:min(560px,92vw);text-align:center";
  card.innerHTML =
    `<h1 style="margin:0 0 4px;font-size:34px;letter-spacing:.5px">onlytransits</h1>` +
    `<div style="color:#9aa3ad;margin-bottom:22px">Build a transit network on a real map. Pick a city.</div>`;

  let selected: CityEntry = CITIES[0];
  let withNetwork = true;

  const grid = document.createElement("div");
  grid.style.cssText = "display:flex;gap:12px;justify-content:center;margin-bottom:20px;flex-wrap:wrap";
  const cards = new Map<string, HTMLButtonElement>();
  for (const c of CITIES) {
    const b = document.createElement("button");
    b.dataset.testid = `city-${c.id}`;
    b.style.cssText =
      "flex:1;min-width:150px;padding:16px;border-radius:12px;border:2px solid transparent;" +
      "background:#1c232b;color:#eef1f4;cursor:pointer;text-align:left";
    b.innerHTML = `<div style="font:600 17px system-ui">${c.name}</div><div style="color:#9aa3ad;font-size:12px;margin-top:4px">${c.blurb}</div>`;
    b.addEventListener("click", () => {
      selected = c;
      syncCards();
    });
    cards.set(c.id, b);
    grid.appendChild(b);
  }
  const syncCards = () => {
    for (const [id, b] of cards) {
      b.style.borderColor = selected.id === id ? "#0aa1dd" : "transparent";
      b.style.background = selected.id === id ? "#11405a" : "#1c232b";
    }
  };
  syncCards();
  card.appendChild(grid);

  // Start mode toggle.
  const modeRow = document.createElement("div");
  modeRow.style.cssText = "display:flex;gap:10px;justify-content:center;margin-bottom:22px";
  const mkMode = (label: string, testid: string, val: boolean) => {
    const b = document.createElement("button");
    b.textContent = label;
    b.dataset.testid = testid;
    b.style.cssText = "padding:9px 14px;border-radius:9px;border:1px solid #39414a;background:#1c232b;color:#eef1f4;cursor:pointer";
    b.addEventListener("click", () => {
      withNetwork = val;
      syncMode();
    });
    return b;
  };
  const netBtn = mkMode("Start with the real network", "mode-network", true);
  const sandboxBtn = mkMode("Empty sandbox", "mode-sandbox", false);
  const syncMode = () => {
    netBtn.style.background = withNetwork ? "#11405a" : "#1c232b";
    netBtn.style.borderColor = withNetwork ? "#0aa1dd" : "#39414a";
    sandboxBtn.style.background = !withNetwork ? "#11405a" : "#1c232b";
    sandboxBtn.style.borderColor = !withNetwork ? "#0aa1dd" : "#39414a";
  };
  syncMode();
  modeRow.append(netBtn, sandboxBtn);
  card.appendChild(modeRow);

  const start = document.createElement("button");
  start.textContent = "▶ Start";
  start.dataset.testid = "start";
  start.style.cssText =
    "padding:12px 28px;border:0;border-radius:10px;background:#0aa1dd;color:#fff;font:600 16px system-ui;cursor:pointer";
  start.addEventListener("click", () => {
    overlay.remove();
    onStart(selected, withNetwork);
  });
  card.appendChild(start);

  overlay.appendChild(card);
  document.body.appendChild(overlay);
}
