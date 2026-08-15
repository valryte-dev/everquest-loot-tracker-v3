export interface BootstrapStatus {
  appVersion: string;
  platform: string;
  databasePath: string;
  databaseReady: boolean;
  schemaVersion: number;
  legacyDatabase: boolean;
}

export type LoadingState<T> =
  | { kind: "loading" }
  | { kind: "ready"; value: T }
  | { kind: "error"; message: string };
