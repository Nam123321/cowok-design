// @story 06.4
// @implements AC-01
import React from "react";
import { PluginCenterPage } from "./pages/plugins/PluginCenterPage";
import { PresenterConsoleWindow } from "./features/flow/components/PresenterConsoleWindow";
import { WorkbenchShell } from "./pages/workbench/WorkbenchShell";
import "./styles/tokens.css";

function resolvePresenterDeckIdFromLocation() {
  if (typeof window === "undefined") return null;
  const match = /^\/presenter\/([^/]+)\/?$/.exec(window.location.pathname);
  if (!match || !match[1]) return null;
  try {
    return decodeURIComponent(match[1]);
  } catch {
    return match[1];
  }
}

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
  const presenterDeckId = resolvePresenterDeckIdFromLocation();

  return (
    <div id="cowok-app-container">
      <div
        id="watchmen-nonce-marker"
        data-watchmen-nonce={nonce}
        style={{ display: "none" }}
      >
        {nonce}
      </div>
      {presenterDeckId ? (
        <PresenterConsoleWindow deckId={presenterDeckId} />
      ) : isPluginCenterRoute ? (
        <PluginCenterPage />
      ) : (
        <WorkbenchShell />
      )}
    </div>
  );
}
