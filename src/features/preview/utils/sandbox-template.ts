import { FixtureVariant } from "../../../lib/validations/screen-fixture";

export const MESSAGE_TYPE = "FLOW_PREVIEW_HYDRATE";

const CSS_STYLES = `
  body {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    background: #0f172a;
    color: #f8fafc;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    padding: 24px;
    box-sizing: border-box;
  }
  .card {
    background: #1e293b;
    border: 1px solid #334155;
    border-radius: 12px;
    padding: 24px;
    max-width: 480px;
    width: 100%;
    box-shadow: 0 10px 25px rgba(0,0,0,0.3);
  }
  .badge {
    display: inline-block;
    padding: 4px 8px;
    border-radius: 6px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    background: #3b82f6;
    color: white;
    margin-bottom: 12px;
  }
  h2 { margin: 0 0 8px 0; font-size: 20px; }
  p { margin: 0 0 16px 0; color: #94a3b8; font-size: 14px; }
  pre {
    background: #090d16;
    padding: 12px;
    border-radius: 8px;
    font-size: 12px;
    overflow-x: auto;
    color: #38bdf8;
  }
`;

export function buildSandboxSrcDoc(
  screenKey: string,
  title: string,
  variant: FixtureVariant,
  payloadStr: string
): string {
  return `<!DOCTYPE html><html><head><meta charset="utf-8"/><style>${CSS_STYLES}</style></head><body><div class="card"><span class="badge" id="variant-badge">${variant}</span><h2>${title}</h2><p>Key: <code>${screenKey}</code></p><pre id="payload-view">${payloadStr}</pre></div><script>window.addEventListener("message",function(e){if(!e.data||e.data.type!=="${MESSAGE_TYPE}")return;var b=document.getElementById("variant-badge");if(b)b.textContent=e.data.variant;var pv=document.getElementById("payload-view");if(pv)pv.textContent=JSON.stringify(e.data.payload,null,2);});</script></body></html>`;
}
