import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import { DamageMeterWindow } from "./app/damage/DamageMeterWindow";
import { SpawnTimerWindow } from "./app/spawn-timers/SpawnTimerWindow";
import { CrashBoundary } from "./app/CrashBoundary";
import { recordClientDiagnostic } from "./shared/backend";
import "./styles/tokens.css";
import "./styles/app.css";

const widget=new URLSearchParams(window.location.search).get("widget");

window.addEventListener("error",event=>{void recordClientDiagnostic("window.error",event.message,event.error?.stack)});
window.addEventListener("unhandledrejection",event=>{const reason=event.reason;void recordClientDiagnostic("promise.rejection",reason instanceof Error?reason.message:String(reason),reason instanceof Error?reason.stack:undefined)});

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <CrashBoundary>{widget==="damage-meter"?<DamageMeterWindow/>:widget==="current-fight"?<DamageMeterWindow focusCurrent/>:widget==="spawn-timers"?<SpawnTimerWindow/>:<App />}</CrashBoundary>
  </StrictMode>,
);
