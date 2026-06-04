// Ambient window flags + debug/test handles. e2e waits on the readiness flags and (T18)
// places stations via the camera-independent __ot_test hook that routes through coords/geo.ts.
import type { Map as MlMap } from "maplibre-gl";
import type { MapboxOverlay } from "@deck.gl/mapbox";
import type { SimBridge } from "./sim/SimBridge";
import type { LoadedCity } from "./sim/city";

declare global {
  interface Window {
    __APP_READY?: boolean;
    __MAP_READY?: boolean;
    __ot?: {
      map: MlMap;
      bridge: SimBridge;
      city: LoadedCity;
      overlay: MapboxOverlay;
    };
    __ot_test?: {
      placeStationLngLat(lng: number, lat: number): void;
      drawLine(stationIds: number[]): void;
      assignTrainset(line: number, count: number): void;
      setHeadwayMs(line: number, ms: number): void;
      setRunning(running: boolean): void;
      stats(): unknown;
    };
  }
}

export {};
