import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";
import { installErrorForwarding } from "./lib/logging";
import { HudView } from "./views/Hud";
import { RegionView } from "./views/RegionSelector";

installErrorForwarding();

// The same bundle serves the main window and the two small transparent windows.
const view = new URLSearchParams(location.search).get("view");
if (view) document.documentElement.classList.add("overlay");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{view === "hud" ? <HudView /> : view === "region" ? <RegionView /> : <App />}</React.StrictMode>,
);
