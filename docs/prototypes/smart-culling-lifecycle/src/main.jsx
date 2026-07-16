import React from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App.jsx";
import "./styles/tokens.css";
import "./styles/shell.css";
import "./styles/screens.css";
import "./styles/review.css";
import "./styles/responsive.css";

createRoot(document.getElementById("root")).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
