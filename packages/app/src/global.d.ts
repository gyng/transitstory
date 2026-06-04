// Ambient window flags used as deterministic readiness signals for Playwright e2e.
// (Test-only placement hooks etc. are added in later tasks; see PLAN §0 / AGENTS testing.)
declare global {
  interface Window {
    __APP_READY?: boolean;
    __MAP_READY?: boolean;
  }
}

export {};
