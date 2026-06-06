import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { BuilderPanelApp } from "./views/BuilderPanelApp";
import "./styles.css";

const rootElement = document.getElementById("root");

if (rootElement === null) {
  throw new Error("无法找到 React 挂载节点");
}

createRoot(rootElement).render(
  <StrictMode>
    <BuilderPanelApp />
  </StrictMode>,
);
