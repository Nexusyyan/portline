import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { TrayPopup } from "./features/tray/TrayPopup";
import "./index.css";

const isTrayView =
  typeof window !== "undefined" &&
  new URLSearchParams(window.location.search).get("view") === "tray";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{isTrayView ? <TrayPopup /> : <App />}</React.StrictMode>
);
