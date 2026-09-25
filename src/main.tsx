import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import { DamageMeterWindow } from "./app/damage/DamageMeterWindow";
import "./styles/tokens.css";
import "./styles/app.css";

const widget=new URLSearchParams(window.location.search).get("widget");

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    {widget==="damage-meter"?<DamageMeterWindow/>:widget==="current-fight"?<DamageMeterWindow focusCurrent/>:<App />}
  </StrictMode>,
);
