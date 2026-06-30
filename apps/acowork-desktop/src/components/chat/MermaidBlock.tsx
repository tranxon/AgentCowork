import { useEffect, useRef, useLayoutEffect, useState } from "react";
import mermaid from "mermaid";

/** (Re-)initialize mermaid global config. Safe to call multiple times. */
function ensureInit() {
  mermaid.initialize({
    startOnLoad: false,
    theme: "base",
    themeVariables: {
      background: "#ffffff",
      primaryColor: "#f8fafc",
      primaryBorderColor: "#cbd5e1",
      primaryTextColor: "#334155",
      lineColor: "#94a3b8",
      secondaryColor: "#f0fdf5",
      tertiaryColor: "#fdfaf5",
      clusterBkg: "#f8fafc",
      clusterBorder: "#d1d5db",
      edgeLabelBackground: "#ffffff",
      nodeBorder: "#cbd5e1",
      nodeTextColor: "#334155",
      fontSize: "12px",
      fontFamily: "system-ui, -apple-system, sans-serif",
      nodeBorderRadius: 12,
    },
    // Inject custom CSS for rounded corners and muted hierarchy colors
    themeCSS: [
      ".node.default > rect,",
      ".node.default > .label-container,",
      ".node > rect,",
      ".node > .label-container {",
      "  rx: 12px !important;",
      "  ry: 12px !important;",
      "}",
      ".node.default > rect,",
      ".node > rect {",
      "  fill: #f8fafc !important;",
      "  stroke: #cbd5e1 !important;",
      "}",
      ".cluster > g > .node.default > rect,",
      ".cluster > g > .node > rect {",
      "  fill: #f0fdf5 !important;",
      "  stroke: #a7c2b4 !important;",
      "}",
      ".cluster > g > .cluster > g > .node.default > rect,",
      ".cluster > g > .cluster > g > .node > rect {",
      "  fill: #fdfaf5 !important;",
      "  stroke: #c4b8a8 !important;",
      "}",
      ".cluster > g > .cluster > g > .cluster > g > .node.default > rect,",
      ".cluster > g > .cluster > g > .cluster > g > .node > rect {",
      "  fill: #f8f6fc !important;",
      "  stroke: #bdb8c8 !important;",
      "}",
      ".label-container {",
      "  border-radius: 12px !important;",
      "}",
    ].join("\n"),
    flowchart: {
      useMaxWidth: true,
      htmlLabels: true,
      curve: "basis",
      padding: 6,
      nodeSpacing: 35,
      rankSpacing: 35,
    },
    sequence: {
      useMaxWidth: true,
      showSequenceNumbers: false,
    },
  });
}

/** Simple non-crypto hash for stable mermaid IDs. */
function hashStr(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) {
    h = ((h << 5) - h + s.charCodeAt(i)) | 0;
  }
  return h;
}

const wrapperClass =
  "my-2 w-full overflow-x-auto rounded-md border border-chat-border bg-chat-body";

/** Applied to the SVG-rendered container — forces SVG to fill available width */
const svgContainerClass =
  "[&_svg]:w-full [&_svg]:max-w-full";

/**
 * Quick validation: skip rendering if the content is clearly incomplete.
 * ReactMarkdown fires the pre component on every streaming chunk, so partial
 * code (e.g. just "sequenceDiagram\n    part") would otherwise trigger a
 * cascade of parse errors from mermaid.render().
 */
function isPlausibleMermaid(code: string): boolean {
  const trimmed = code.trim();
  if (!trimmed) return false;

  // Needs at least a header line + one content line
  const lines = trimmed.split("\n");
  if (lines.length < 2) return false;

  const firstLine = lines[0].trim();
  const supported = [
    "flowchart",
    "graph",
    "sequenceDiagram",
    "classDiagram",
    "stateDiagram",
    "stateDiagram-v2",
    "erDiagram",
    "gantt",
    "pie",
    "gitGraph",
    "mindmap",
    "timeline",
    "quadrantChart",
    "xyChart",
    "block",
    "architecture",
    "kanban",
    "sankey",
    "xychart",
  ];
  if (!supported.some((t) => firstLine.startsWith(t))) return false;

  // Skip if the last non-empty line looks incomplete — it ends with an
  // edge marker that expects more tokens on the same line (streaming).
  const lastNonEmpty = [...lines].reverse().find((l) => l.trim().length > 0);
  if (lastNonEmpty) {
    const endsWithPartial = /(?:-->|->|==>|=>|-\.>$|--x|--o)$/.test(lastNonEmpty.trim());
    if (endsWithPartial) return false;
  }

  return true;
}

// Module-level height cache backed by sessionStorage so cached heights
// survive page refreshes within the same browser tab. sessionStorage is
// cleared when the tab closes, so there is no unbounded accumulation.
const heightCache = {
  prefix: "mermaid-h:",
  get(key: string): number | undefined {
    try {
      const v = sessionStorage.getItem(`${this.prefix}${key}`);
      return v ? Number(v) : undefined;
    } catch {
      return undefined;
    }
  },
  set(key: string, h: number) {
    try {
      sessionStorage.setItem(`${this.prefix}${key}`, String(h));
    } catch {
      // sessionStorage full or unavailable — silently ignore
    }
  },
};

interface MermaidBlockProps {
  chart: string;
}

export function MermaidBlock({ chart }: MermaidBlockProps) {
  const instanceIdRef = useRef(`m-${Math.random().toString(36).slice(2, 8)}`);
  const [svgContent, setSvgContent] = useState<string | null>(null);
  const [renderFailed, setRenderFailed] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  // Stable cache key derived from chart content
  const cacheKey = `h-${hashStr(chart)}`;
  const cachedHeight = heightCache.get(cacheKey);

  useEffect(() => {
    // Debounce: wait 200ms for the stream to settle before attempting render.
    // Rapid prop updates during streaming would otherwise fire a cascade of
    // partial renders, each failing on incomplete content.
    const timer = setTimeout(() => {
      // Don't attempt render on partial streaming chunks
      if (!isPlausibleMermaid(chart)) return;

      ensureInit();

      let cancelled = false;
      const id = `${instanceIdRef.current}-${hashStr(chart)}`;

      (async () => {
        try {
          const { svg } = await mermaid.render(id, chart);
          if (!cancelled) {
            setSvgContent(svg);
            setRenderFailed(false);
          }
        } catch (err) {
          console.error("[MermaidBlock] render failed:", err);
          console.error("[MermaidBlock] chart content:", chart.slice(0, 500));
          if (!cancelled) {
            setSvgContent(null);
            setRenderFailed(true);
          }
        }
      })();
    }, 200);

    return () => {
      clearTimeout(timer);
    };
  }, [chart]);

  // After any state transition (loading → success, loading → error, or
  // remount with cached height), measure the container and lock its height.
  // Only final states (svgContent or renderFailed) are written to the cache
  // so that remounts skip the loading placeholder height.
  useLayoutEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    requestAnimationFrame(() => {
      if (!el) return;
      const h = el.offsetHeight;
      if (h > 0) {
        el.style.minHeight = `${h}px`;
        if (svgContent || renderFailed) {
          heightCache.set(cacheKey, h);
        }
      }
    });
  }, [svgContent, renderFailed, cacheKey]);

  // Unified container — same div, same ref across all three states.
  // The virtualizer's ResizeObserver stays bound to this single element,
  // and the cached minHeight (when available) prevents any initial jump.
  return (
    <div
      ref={containerRef}
      className={`${wrapperClass} ${svgContent ? `${svgContainerClass} [&_.label]:text-zinc-600` : ""} p-3`}
      style={cachedHeight ? { minHeight: `${cachedHeight}px` } : undefined}
    >
      {svgContent ? (
        <div dangerouslySetInnerHTML={{ __html: svgContent }} />
      ) : renderFailed ? (
        <pre className="m-0 whitespace-pre-wrap font-mono text-xs leading-relaxed text-zinc-500 dark:text-zinc-400">
          {chart}
        </pre>
      ) : (
        <div className="min-h-[140px] flex items-center justify-center">
          <div className="flex items-center gap-2 text-zinc-300 dark:text-zinc-500 select-none">
            <svg
              className="h-4 w-4 animate-spin"
              xmlns="http://www.w3.org/2000/svg"
              fill="none"
              viewBox="0 0 24 24"
            >
              <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
              <path
                className="opacity-75"
                fill="currentColor"
                d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
              />
            </svg>
            <span className="text-xs">Rendering diagram...</span>
          </div>
        </div>
      )}
    </div>
  );
}
