// App entry. T3: app shell + readiness flag for the Playwright load smoke. T5 mounts the
// MapLibre map here and flips __MAP_READY on map 'idle'.
import "./styles.css";

const title = document.createElement("h1");
title.id = "app-title";
title.textContent = "onlytransits";
title.style.cssText =
  "position:fixed;top:12px;left:16px;margin:0;font:600 18px system-ui,sans-serif;color:#1c2024;z-index:10";
document.getElementById("ui")?.appendChild(title);

window.__APP_READY = true;
