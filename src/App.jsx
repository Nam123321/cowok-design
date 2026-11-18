import React from "react";
import { PluginCenterPage } from "./pages/plugins/PluginCenterPage";
import { WorkbenchShell } from "./pages/workbench/WorkbenchShell";
import "./styles/tokens.css";

export default function App() {
  const nonce =
    (typeof import.meta !== "undefined" &&
      import.meta.env &&
      (import.meta.env.VITE_WATCHMEN_NONCE ||
        import.meta.env.VITE_QA_NONCE ||
        import.meta.env.WATCHMEN_NONCE)) ||
    "";
  const pathname = typeof window !== "undefined" ? window.location.pathname : "";
  const isPluginCenterRoute = pathname === "/plugins" || pathname === "/plugins/";

  return (
    <div id="cowok-app-container">
      <div
        id="watchmen-nonce-marker"
        data-watchmen-nonce={nonce}
        style={{ display: "none" }}
      >
        {nonce}
      </div>
      {isPluginCenterRoute ? <PluginCenterPage /> : <WorkbenchShell />}
    </div>
  );
}
