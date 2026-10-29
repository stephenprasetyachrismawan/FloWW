import React from "react";
import ReactDOM from "react-dom/client";

import "@fontsource-variable/plus-jakarta-sans";
import "@fontsource-variable/source-serif-4";
import "@fontsource-variable/jetbrains-mono";

import App from "./App";
import { applyAppearance, loadAppearance } from "./lib/appearance";
import { Updater } from "./pages/updater";
import "./index.css";

/** Set by the desktop app when this window is the updater. */
const isUpdater = (window as unknown as { __FLOWW_UPDATER__?: boolean }).__FLOWW_UPDATER__ === true;

applyAppearance(loadAppearance());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{isUpdater ? <Updater /> : <App />}</React.StrictMode>,
);
