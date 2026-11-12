import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";
import { installErrorForwarding } from "./lib/logging";
import { HudView } from "./views/Hud";
import { QuickCaptureView } from "./views/QuickCapture";
import { RegionView } from "./views/RegionSelector";

installErrorForwarding();

// The same bundle serves the main window and the overlay windows (HUD, area picker, quick capture).
const view = new URLSearchParams(location.search).get("view");
if (view) document.documentElement.classList.add("overlay");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{view === "hud" ? <HudView /> : view === "region" ? <RegionView /> : view === "quick" ? <QuickCaptureView /> : <App />}</React.StrictMode>,
);
