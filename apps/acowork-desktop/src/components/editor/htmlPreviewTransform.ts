/** Utilities for rendering workspace HTML files in a sandboxed iframe. */

const ROOT_RELATIVE_ATTRS = ["src", "href", "poster", "action"];
const SKIP_URL_RE = /^(?:[a-z][a-z0-9+.-]*:|#|\/\/)/i;

export interface HtmlPreviewTransformOptions {
    content: string;
    gatewayUrl: string;
    agentId: string;
    workspaceId: string;
    relPath: string;
}

export interface HtmlPreviewTransformResult {
    html: string;
    baseHref: string;
    workspaceAssetRoot: string;
    hasTsModuleEntry: boolean;
}

function encodePathSegments(path: string): string {
    return path
        .split("/")
        .filter((segment) => segment.length > 0)
        .map(encodeURIComponent)
        .join("/");
}

function getDirname(relPath: string): string {
    const normalized = relPath.replace(/\\/g, "/");
    const idx = normalized.lastIndexOf("/");
    return idx >= 0 ? normalized.slice(0, idx) : "";
}

export function buildWorkspaceAssetRoot(gatewayUrl: string, agentId: string, workspaceId: string): string {
    const base = gatewayUrl.replace(/\/+$/g, "");
    return `${base}/workspace-files/${encodeURIComponent(agentId)}/${encodeURIComponent(workspaceId)}/`;
}

export function buildHtmlBaseHref(gatewayUrl: string, agentId: string, workspaceId: string, relPath: string): string {
    const root = buildWorkspaceAssetRoot(gatewayUrl, agentId, workspaceId);
    const dir = encodePathSegments(getDirname(relPath));
    return dir ? `${root}${dir}/` : root;
}

function shouldRewriteUrl(value: string): boolean {
    const trimmed = value.trim();
    if (!trimmed.startsWith("/")) return false;
    if (SKIP_URL_RE.test(trimmed.slice(1))) return false;
    return true;
}

function rewriteRootRelativeAttrs(html: string, workspaceAssetRoot: string): string {
    let out = html;
    for (const attr of ROOT_RELATIVE_ATTRS) {
        const re = new RegExp(`\\b(${attr})(\\s*=\\s*)(["'])(/[^"']*)\\3`, "gi");
        out = out.replace(re, (_match, name: string, eq: string, quote: string, value: string) => {
            if (!shouldRewriteUrl(value)) return _match;
            const rewritten = `${workspaceAssetRoot}${encodePathSegments(value.replace(/^\/+/, ""))}`;
            return `${name}${eq}${quote}${rewritten}${quote}`;
        });
    }
    return out;
}

function rewriteSrcset(html: string, workspaceAssetRoot: string): string {
    return html.replace(/\b(srcset)(\s*=\s*)(["'])([^"']*)\3/gi, (_match, name: string, eq: string, quote: string, value: string) => {
        const rewritten = value
            .split(",")
            .map((candidate) => {
                const trimmed = candidate.trim();
                if (!trimmed) return candidate;
                const parts = trimmed.split(/\s+/);
                const url = parts[0];
                if (!shouldRewriteUrl(url)) return candidate;
                parts[0] = `${workspaceAssetRoot}${encodePathSegments(url.replace(/^\/+/, ""))}`;
                return parts.join(" ");
            })
            .join(", ");
        return `${name}${eq}${quote}${rewritten}${quote}`;
    });
}

function injectBaseHref(html: string, baseHref: string): string {
    const baseTag = `<base href="${baseHref}">`;
    if (/<base\b/i.test(html)) {
        return html.replace(/<base\b[^>]*>/i, baseTag);
    }
    if (/<head[^>]*>/i.test(html)) {
        return html.replace(/<head[^>]*>/i, (match) => `${match}\n${baseTag}`);
    }
    return `${baseTag}\n${html}`;
}

export function detectTsModuleEntry(html: string): boolean {
    const scriptRe = /<script\b[^>]*\bsrc\s*=\s*(["'])([^"']+\.(?:tsx|ts)(?:[?#][^"']*)?)\1[^>]*>/gi;
    return scriptRe.test(html);
}

export function transformHtmlPreview(options: HtmlPreviewTransformOptions): HtmlPreviewTransformResult {
    const workspaceAssetRoot = buildWorkspaceAssetRoot(options.gatewayUrl, options.agentId, options.workspaceId);
    const baseHref = buildHtmlBaseHref(options.gatewayUrl, options.agentId, options.workspaceId, options.relPath);
    const hasTsModuleEntry = detectTsModuleEntry(options.content);

    let html = injectBaseHref(options.content, baseHref);
    html = rewriteRootRelativeAttrs(html, workspaceAssetRoot);
    html = rewriteSrcset(html, workspaceAssetRoot);

    return { html, baseHref, workspaceAssetRoot, hasTsModuleEntry };
}

// Lightweight self-test used by `npm run check:html-preview`.
export function runHtmlPreviewTransformSelfTest(): void {
    const result = transformHtmlPreview({
        content: `<!doctype html><html><head></head><body>
<script type="module" src="/src/main.tsx"></script>
<script src="./local.js"></script>
<img src="/assets/logo.png" srcset="/a.png 1x, /b.png 2x, https://cdn.example.com/c.png 3x">
<a href="#anchor">anchor</a><a href="https://example.com">external</a>
</body></html>`,
        gatewayUrl: "http://127.0.0.1:19876/",
        agentId: "agent/a",
        workspaceId: "ws 1",
        relPath: "pages/demo/index.html",
    });

    const expectedRoot = "http://127.0.0.1:19876/workspace-files/agent%2Fa/ws%201/";
    const checks: Array<[boolean, string]> = [
        [result.baseHref === `${expectedRoot}pages/demo/`, "base href should point at the HTML file directory"],
        [result.html.includes(`src="${expectedRoot}src/main.tsx"`), "root-relative script src should be rewritten"],
        [result.html.includes('src="./local.js"'), "relative script src should stay relative"],
        [result.html.includes(`src="${expectedRoot}assets/logo.png"`), "root-relative image src should be rewritten"],
        [result.html.includes(`srcset="${expectedRoot}a.png 1x, ${expectedRoot}b.png 2x`) && result.html.includes("https://cdn.example.com/c.png 3x"), "root-relative srcset candidates should be rewritten"],
        [result.html.includes('href="#anchor"'), "hash href should not be rewritten"],
        [result.html.includes('href="https://example.com"'), "external href should not be rewritten"],
        [result.hasTsModuleEntry, "TS/TSX module entry should be detected"],
    ];

    const failed = checks.filter(([ok]) => !ok).map(([, message]) => message);
    if (failed.length > 0) {
        throw new Error(`HtmlPreview transform self-test failed:\n- ${failed.join("\n- ")}`);
    }
}
