// Ambient window flags + debug/test handles. e2e waits on the readiness flags and (T18)
// places stations via the camera-independent __ot_test hook that routes through coords/geo.ts.
import type { Map as MlMap } from "maplibre-gl";
import type { MapboxOverlay } from "@deck.gl/mapbox";
import type { SimBridge } from "./sim/SimBridge";
import type { LoadedCity } from "./sim/city";
import type { Game } from "./game";

declare global {
  interface Window {
    __APP_READY?: boolean;
    __MAP_READY?: boolean;
    __ot?: {
      map: MlMap;
      bridge: SimBridge;
      city: LoadedCity;
      overlay: MapboxOverlay;
      game: Game;
    };
    __ot_test?: {
      tickMs(ms: number): void;
      placeStationLngLat(lng: number, lat: number): number;
      placeSignalLngLat(lng: number, lat: number): boolean;
      placedSignalIds(): string[];
      placeBarracksLngLat(lng: number, lat: number): number;
      postBounty(station: number, amount: number): void;
      unlockTech(tech: number): void;
      castSpell(kind: number): void;
      setAutocast(enabled: boolean): void;
      drawLine(stationIds: number[]): number;
      selectLine(id: number | null): void;
      setLineTrack(line: number, track: number): void;
      assignTrainset(line: number, count: number): void;
      setHeadwayMs(line: number, ms: number): void;
      setRunning(running: boolean): void;
      setSpeed(mult: number): void;
      setLineMode(line: number, mode: number): void;
      setTransport(mode: number): void;
      setModeEnabled(mode: number, on: boolean): void;
      setShowDemand(on: boolean): void;
      setShowReach(on: boolean): void;
      setShowRoads(on: boolean): void;
      stationTip(id: number): import("./ui/react/shared").StationTip | null;
      lineTip(id: number): import("./ui/react/shared").LineTip | null;
      vehicleTip(index: number): import("./ui/react/shared").VehicleTip | null;
      stats(): unknown;
    };
  }
}

export {};
