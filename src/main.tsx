import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource/inter/latin-400.css";
import "@fontsource/inter/latin-500.css";
import App from "./App";
import "./styles/app.css";
import "./styles/motion.css";
// Transparent web content lets the native macOS menu material show through.
if (new URLSearchParams(location.search).get("view") === "tray") {
  document.documentElement.dataset.surface = "tray";
}
ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
