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

interface MermaidBlockProps {
  chart: string;
}

export function MermaidBlock({ chart }: MermaidBlockProps) {
  const instanceIdRef = useRef(`m-${Math.random().toString(36).slice(2, 8)}`);
  const [svgContent, setSvgContent] = useState<string | null>(null);
  const [renderFailed, setRenderFailed] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
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
      } catch {
        if (!cancelled) {
          setSvgContent(null);
          setRenderFailed(true);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [chart]);

  // After SVG is injected, measure and lock the height so the virtualizer
  // doesn't see further re-layouts on subsequent renders.
  useLayoutEffect(() => {
    if (svgContent && containerRef.current) {
      const el = containerRef.current;
      // Wait a microtask for the SVG to render, then measure
      requestAnimationFrame(() => {
        if (el) {
          const h = el.offsetHeight;
          if (h > 0) {
            el.style.minHeight = `${h}px`;
          }
        }
      });
    }
  }, [svgContent]);

  if (renderFailed) {
    return (
      <div className={`${wrapperClass} p-3`}>
        <pre className="m-0 whitespace-pre-wrap font-mono text-xs leading-relaxed text-zinc-500 dark:text-zinc-400">
          {chart}
        </pre>
      </div>
    );
  }

  // Loading state: show a placeholder with visible min-height so the
  // virtualizer has a stable measure, avoiding scroll jank when the
  // async mermaid.render() finally injects the SVG.
  if (!svgContent) {
    return (
      <div
        ref={containerRef}
        className={`${wrapperClass} min-h-[140px] flex items-center justify-center p-3`}
      >
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
    );
  }

  return (
    <div
      ref={containerRef}
      className={`${wrapperClass} ${svgContainerClass} [&_.label]:text-zinc-600 p-3`}
      dangerouslySetInnerHTML={{ __html: svgContent }}
    />
  );
}
