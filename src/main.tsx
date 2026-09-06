import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import { DamageMeterWindow } from "./app/damage/DamageMeterWindow";
import "./styles/tokens.css";
import "./styles/app.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    {new URLSearchParams(window.location.search).get("widget")==="damage-meter"?<DamageMeterWindow/>:<App />}
  </StrictMode>,
);
