import { invoke } from "@tauri-apps/api/core";
import type { BootstrapStatus } from "./contracts";

const browserPreview: BootstrapStatus = {
  appVersion: "0.1.0",
  platform: "browser-preview",
  databasePath: "Available when running inside the V3 desktop shell",
  databaseReady: false,
  schemaVersion: 0,
  legacyDatabase: false,
};

export async function bootstrapStatus(): Promise<BootstrapStatus> {
  if (!("__TAURI_INTERNALS__" in window)) return browserPreview;
  return invoke<BootstrapStatus>("bootstrap_status");
}
