// App entry. T1: minimal shell that mounts a title and sets a readiness flag so the
// Playwright load smoke has a deterministic signal. T5 replaces this with the map.

const title = document.createElement("h1");
title.id = "app-title";
title.textContent = "onlytransits";
title.style.cssText =
  "position:fixed;top:12px;left:16px;margin:0;font:600 18px system-ui,sans-serif;color:#222;z-index:10";
document.getElementById("ui")?.appendChild(title);

window.__APP_READY = true;
