#!/usr/bin/env node
// scripts/check-i18n.mjs
//
// Compile-time lint for i18next translation files.
//
// Why this exists:
//   i18next default interpolation prefix/suffix is `{{` / `}}` (double braces).
//   AI-generated translations have repeatedly introduced SINGLE-brace
//   placeholders like `{count}` which i18next silently fails to interpolate
//   — they render as literal `{count}` text in the UI. To make the failure
//   loud and fast, this script enforces the double-brace convention at build
//   time and exits non-zero on any violation.
//
// Rules enforced per translation value:
//   1. `{{name}}` (double braces)  — ALLOWED
//   2. `{name}`   (single braces)  — ERROR  (most common AI mistake)
//   3. `{{{name}}}` (triple braces) — ERROR  (over-escape / typo)
//   4. Unbalanced braces like `{name` or `name}` — ERROR
//   5. Empty braces `{}` or `{{}}` — ERROR
//
// Usage:
//   node scripts/check-i18n.mjs
//   npm run check:i18n
//
// Exit code: 0 on success, 1 on any violation.

import { readFileSync, readdirSync } from "node:fs";
import { join, relative, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// i18n locale directory is at <projectRoot>/src/i18n/locales
const LOCALES_DIR = resolve(__dirname, "..", "src", "i18n", "locales");

/**
 * Walk a plain object recursively, calling `visit(value, path)` for every
 * leaf string value. `path` is a dot-separated key path (e.g. "time.minutesAgo").
 */
function walk(obj, path, visit) {
    if (obj === null || obj === undefined) return;
    if (typeof obj === "string") {
        visit(obj, path);
        return;
    }
    if (Array.isArray(obj)) {
        obj.forEach((item, i) => walk(item, `${path}[${i}]`, visit));
        return;
    }
    if (typeof obj === "object") {
        for (const [k, v] of Object.entries(obj)) {
            const childPath = path ? `${path}.${k}` : k;
            walk(v, childPath, visit);
        }
    }
}

/**
 * Inspect a translation value and return an array of violation messages.
 * Each message describes a single bad placeholder occurrence.
 */
function findViolations(value) {
    const errors = [];
    // Match runs of `{` and `}` so we can detect unbalanced or wrong-arity braces.
    // We scan for any sequence containing `{` and `}` and analyze the run.
    const re = /\{+[^{}]*\}+/g;
    let m;
    while ((m = re.exec(value)) !== null) {
        const token = m[0];
        const openCount = (token.match(/^\{+/) || [""])[0].length;
        const closeCount = (token.match(/\}+$/) || [""])[0].length;
        const inner = token.slice(openCount, token.length - closeCount);
        const ctx = value.slice(Math.max(0, m.index - 20), m.index + token.length + 20);

        if (openCount !== closeCount) {
            errors.push(
                `Unbalanced braces \`${token}\` (open=${openCount}, close=${closeCount}) in "${truncate(ctx)}"`,
            );
        } else if (openCount === 1) {
            errors.push(
                `Single-brace placeholder \`${token}\` is not valid i18next syntax. Use double braces: \`{{${inner.trim()}}}\`. Context: "${truncate(ctx)}"`,
            );
        } else if (openCount === 3) {
            // i18next triple-brace is for raw HTML and almost never wanted.
            errors.push(
                `Triple-brace placeholder \`${token}\` is almost certainly a typo. Use \`{{${inner.trim()}}}\`. Context: "${truncate(ctx)}"`,
            );
        } else if (openCount >= 4) {
            errors.push(
                `Quad+ brace placeholder \`${token}\` is invalid i18next syntax. Context: "${truncate(ctx)}"`,
            );
        } else if (openCount === 2) {
            // Double braces — check that the inner is a valid identifier-ish token.
            if (inner.trim().length === 0) {
                errors.push(`Empty double-brace placeholder \`{{}}\` in "${truncate(ctx)}"`);
            } else if (!/^[a-zA-Z_][a-zA-Z0-9_]*$/.test(inner.trim())) {
                errors.push(
                    `Invalid placeholder name \`{{${inner.trim()}}}\` in "${truncate(ctx)}" (expected identifier like {{count}})`,
                );
            }
        }
    }
    return errors;
}

function truncate(s, max = 60) {
    if (s.length <= max) return s;
    return s.slice(0, max - 1) + "…";
}

function main() {
    const files = readdirSync(LOCALES_DIR).filter((f) => f.endsWith(".json"));
    if (files.length === 0) {
        console.error(`[check-i18n] No JSON files found in ${LOCALES_DIR}`);
        process.exit(1);
    }

    let totalErrors = 0;
    const reportLines = [];

    for (const file of files) {
        const filePath = join(LOCALES_DIR, file);
        let data;
        try {
            data = JSON.parse(readFileSync(filePath, "utf8"));
        } catch (e) {
            console.error(`[check-i18n] Failed to parse ${relative(process.cwd(), filePath)}: ${e.message}`);
            process.exit(1);
        }

        const fileErrors = [];
        walk(data, "", (value, path) => {
            const violations = findViolations(value);
            for (const v of violations) {
                fileErrors.push(`  - ${path}: ${v}`);
            }
        });

        if (fileErrors.length > 0) {
            totalErrors += fileErrors.length;
            reportLines.push(`\n[${relative(process.cwd(), filePath)}] ${fileErrors.length} violation(s):`);
            reportLines.push(...fileErrors);
        }
    }

    if (totalErrors === 0) {
        console.log(`[check-i18n] OK — all ${files.length} locale files use valid i18next double-brace placeholders.`);
        process.exit(0);
    }

    console.error("\n[check-i18n] FAILED — i18n placeholder lint errors detected:");
    console.error(reportLines.join("\n"));
    console.error(`\nTotal: ${totalErrors} violation(s) across ${files.length} file(s).`);
    console.error("\nFix: i18next requires DOUBLE braces `{{name}}`, not single `{name}`.");
    console.error("Single braces render as literal text and (if followed by uppercase CSS) may look like uppercase placeholders.\n");
    process.exit(1);
}

main();
