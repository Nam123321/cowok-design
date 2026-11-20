// @story 06.3
// @implements AC-03 (Deterministic 2D SVG Chart Sandbox Snippet Builders)
import { DEFAULT_FALLBACK_SLIDE_UID } from "../../../lib/validations/presentation";

function buildChartProportionalSnippet(): string {
  return `    if (spec.kind === "pie" || spec.kind === "donut") {
      var tot = 0; for (var i = 0; i < firstPts.length; i++) tot += Math.max(0, Number(firstPts[i].value) || 0);
      tot = tot || 100; var cx = w * 0.5, cy = h * 0.54, oR = Math.min(w, h) * 0.36, iR = spec.kind === "donut" ? oR * 0.56 : 0, deg = -90;
      for (var pi = 0; pi < firstPts.length && prim < 1990; pi++) {
        var span = (Math.max(0, Number(firstPts[pi].value) || 0) / tot) * 360;
        svg.appendChild(mk("path", { d: chartArcD(cx, cy, oR, iR, deg, deg + span), fill: palette[pi % palette.length] }));
        deg += span;
      }
    } else if (spec.kind === "100_stacked") {
      var stot = 0; for (var si = 0; si < firstPts.length; si++) stot += Math.max(0, Number(firstPts[si].value) || 0);
      stot = stot || 100; var sx = px, iw = w - px * 2;
      for (var sj = 0; sj < firstPts.length && prim < 1990; sj++) {
        var sw = Math.max(1, (Math.max(0, Number(firstPts[sj].value) || 0) / stot) * iw);
        svg.appendChild(mk("rect", { x: chartRound1(sx), y: 86, width: chartRound1(sw), height: 36, fill: palette[sj % palette.length], rx: 3 }));
        sx += sw;
      }
    } else if (spec.kind === "funnel") {
      var fmax = 1; for (var fi = 0; fi < firstPts.length; fi++) fmax = Math.max(fmax, Number(firstPts[fi].value) || 0);
      var fih = h - pt - pb, fiw = w - px * 2, fsh = Math.max(8, fih / Math.max(1, firstPts.length) - 4);
      for (var fj = 0; fj < firstPts.length && prim < 1990; fj++) {
        var fsw = fiw * Math.max(0.28, (Math.max(0, Number(firstPts[fj].value) || 0) / fmax));
        svg.appendChild(mk("rect", { x: chartRound1(px + (fiw - fsw) * 0.5), y: chartRound1(pt + fj * (fsh + 4)), width: chartRound1(fsw), height: chartRound1(fsh), fill: palette[fj % palette.length], rx: 4 }));
      }
    }`;
}

function buildChartCartesianSnippet(): string {
  return `    else {
      var minV = 0, maxV = 0, capSeries = Math.min(series.length, 12);
      for (var ri = 0; ri < capSeries; ri++) {
        var rPts = series[ri].points || [], capPts = Math.min(rPts.length, 200);
        for (var rj = 0; rj < capPts; rj++) { var rv = Number(rPts[rj].value); if (!Number.isFinite(rv)) continue; if (rv < minV) minV = rv; if (rv > maxV) maxV = rv; }
      }
      var spanV = Math.max(1, maxV - minV), ciw = w - px * 2, cih = h - pt - pb, baseY = pt + (maxV / spanV) * cih;
      svg.appendChild(mk("line", { x1: px, y1: chartRound1(baseY), x2: w - px, y2: chartRound1(baseY), stroke: "currentColor", "stroke-opacity": 0.35 }));
      if (spec.kind === "bar") {
        var bCap = Math.min(firstPts.length, 200), bSlot = ciw / Math.max(1, bCap), bw = Math.max(4, bSlot - 4);
        for (var bi = 0; bi < bCap && prim < 1990; bi++) {
          var bv = Number(firstPts[bi].value); if (!Number.isFinite(bv)) bv = 0;
          var bh = Math.max(2, (Math.abs(bv) / spanV) * cih);
          svg.appendChild(mk("rect", { x: chartRound1(px + bi * bSlot + 2), y: chartRound1(bv >= 0 ? baseY - bh : baseY), width: chartRound1(bw), height: chartRound1(bh), fill: palette[bi % palette.length], rx: 3 }));
        }
      } else {
        for (var li = 0; li < capSeries && prim < 1990; li++) {
          var lPts = series[li].points || [], lCap = Math.min(lPts.length, 200), coords = [], col = palette[li % palette.length];
          for (var lj = 0; lj < lCap && prim < 1990; lj++) {
            var lv = Number(lPts[lj].value); if (!Number.isFinite(lv)) lv = 0;
            var lx = lCap === 1 ? px + ciw * 0.5 : px + (lj / (lCap - 1)) * ciw;
            var ly = pt + ((maxV - lv) / spanV) * cih;
            coords.push(chartRound1(lx) + "," + chartRound1(ly));
          }
          if (spec.kind === "area" && coords.length > 0) svg.appendChild(mk("polygon", { points: coords[0].split(",")[0] + "," + chartRound1(baseY) + " " + coords.join(" ") + " " + coords[coords.length - 1].split(",")[0] + "," + chartRound1(baseY), fill: col, "fill-opacity": 0.22 }));
          svg.appendChild(mk("polyline", { points: coords.join(" "), fill: "none", stroke: col, "stroke-width": 2 }));
        }
      }
    }`;
}

function buildChartHeaderSetupSnippet(): string {
  return `  function applyRender2DChart(frame) {
    if (!frame || !frame.ffId || !frame.spec || !document.createElementNS) return;
    var target = resolvePatchTarget(frame.slideUid, String(frame.ffId));
    if (!target) return;
    var started = (typeof performance !== "undefined" && performance.now) ? performance.now() : Date.now();
    var ns = "http://www.w3.org/2000/svg", w = 360, h = 200, px = 28, pt = 24, pb = 26, prim = 0;
    var palette = ["var(--human-accent)", "var(--agent-accent)", "var(--state-success)", "var(--state-info)", "var(--state-warning)", "var(--text-secondary)"];
    function mk(tag, attrs) {
      prim++; var el = document.createElementNS(ns, tag);
      for (var k in attrs) el.setAttribute(k, String(attrs[k]));
      return el;
    }
    var spec = frame.spec, series = Array.isArray(spec.series) ? spec.series : [];
    var svg = document.createElementNS(ns, "svg");
    svg.setAttribute("viewBox", "0 0 " + w + " " + h);
    svg.setAttribute("role", "img");
    svg.setAttribute("aria-label", String(frame.ariaLabel || spec.title || "Chart").slice(0, 320));
    svg.setAttribute("data-chart-kind", String(spec.kind || "bar"));
    svg.style.width = "100%";
    svg.style.maxWidth = "320px";
    svg.style.maxHeight = "140px";
    svg.style.height = "auto";
    svg.style.display = "block";
    var titleEl = mk("text", { x: px, y: 15, fill: "currentColor", "font-size": 11, "font-weight": 600 });
    titleEl.textContent = String(spec.title || ""); svg.appendChild(titleEl);
    var firstPts = (series[0] && Array.isArray(series[0].points)) ? series[0].points : [];`;
}

function buildChartFooterAckSnippet(): string {
  return `    while (target.firstChild) target.removeChild(target.firstChild);
    target.appendChild(svg);
    try { target.setAttribute("data-charts-json", JSON.stringify(spec).slice(0, 65536)); } catch (_) {}
    var r = target.getBoundingClientRect ? target.getBoundingClientRect() : { left: 0, top: 0, width: w, height: h };
    var ended = (typeof performance !== "undefined" && performance.now) ? performance.now() : Date.now();
    scheduleElementRectsPublish();
    if (bridgePort) bridgePort.postMessage({ type: "CHART_RENDER_ACK", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: CHANNEL_SECRET, seq: ++localSeq, slideUid: String(frame.slideUid || "${DEFAULT_FALLBACK_SLIDE_UID}"), ffId: String(frame.ffId), svgPrimitiveCount: Math.min(2000, prim), renderTimeMs: Math.max(0, ended - started), boundsRect: { x: Math.round(r.left || 0), y: Math.round(r.top || 0), width: Math.round(r.width || w), height: Math.round(r.height || h) } });
  }
  function hydrateEmbeddedCharts() {
    if (!document.querySelectorAll) return;
    document.querySelectorAll("[data-ff-id][data-charts-json]").forEach(function(el) {
      var raw = el.getAttribute("data-charts-json") || "";
      var ffId = el.getAttribute("data-ff-id") || "";
      if (!ffId || !raw || raw.trim() === "[]" || raw.length > 65536) return;
      try {
        var parsed = JSON.parse(raw);
        if (!parsed || typeof parsed !== "object" || !parsed.kind || !Array.isArray(parsed.series)) return;
        var hostSlide = el.closest ? el.closest(".slide") : null;
        var uid = (hostSlide && hostSlide.getAttribute ? hostSlide.getAttribute("data-slide-uid") : null) || "${DEFAULT_FALLBACK_SLIDE_UID}";
        applyRender2DChart({ slideUid: uid, ffId: ffId, spec: parsed, ariaLabel: parsed.ariaSummary || parsed.title || "Chart" });
      } catch (_) {}
    });
  }`;
}

export function buildRender2DChartSnippet(arcSnippet: string): string {
  return [
    arcSnippet,
    buildChartHeaderSetupSnippet(),
    buildChartProportionalSnippet(),
    buildChartCartesianSnippet(),
    buildChartFooterAckSnippet(),
  ].join("\n");
}
