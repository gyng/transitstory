// App entry. React owns the floating UI chrome (menu, panels, chorded bar) rendered into
// #ui; the full-screen map (#map) + deck.gl overlay + rAF render loop are imperative and
// live outside React (AGENTS render-hot-path rule). See ui/react/App.tsx for the boot flow.
import "./styles.css";
import { createRoot } from "react-dom/client";
import { App } from "./ui/react/App";

// No <StrictMode>: it double-invokes effects in dev, which would double-boot the imperative
// map/deck overlay and churn the rAF loop. The world is built once, imperatively, in App.boot.
createRoot(document.getElementById("ui")!).render(<App />);
