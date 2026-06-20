import React from "react";
import ReactDOM from "react-dom/client";
import RecordingOverlay from "./RecordingOverlay";
import "./overlay.css";

const root = document.getElementById("overlay-root");
if (root) {
  ReactDOM.createRoot(root).render(
    <React.StrictMode>
      <RecordingOverlay />
    </React.StrictMode>,
  );
}
