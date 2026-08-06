# Security

Preview injection surface, URL sanitization, figures, CSP, capabilities, logging and telemetry.

Related: [ARCHITECTURE](ARCHITECTURE.md) · [PRODUCT](PRODUCT.md)

---

## Threat model

The application works offline and makes no network requests during normal operation, which removes most of the usual surface and leaves one that matters:

> **USFM files are documents received from third parties.** They are exchanged by email, shared drives, and USB. USFM 3.0 added attributes carrying user-controlled URLs. A malicious or merely careless file is the realistic threat.

Secondary, and a confidentiality concern rather than an attack: much of this audience works on unpublished translation under confidentiality obligations. Content must not leak through logs, crash reports, or telemetry.

## 1. The preview never executes content

**`{@html}` appears nowhere in the codebase.** The preview is built from typed model nodes rendered by Svelte components, never from strings. Escaping is not the control — the control is that no path exists from document content to raw markup.

Enforced mechanically, because a convention is not a control:

```javascript
// eslint.config.js
"no-restricted-syntax": [
  "error",
  { selector: "SvelteMustacheTagRaw", message: "{@html} is banned; render typed nodes." }
]
```

plus a CI grep for `{@html}` and `innerHTML` across `src/`.

## 2. URL sanitization

USFM 3.0's `\jmp …|link-href="…"\jmp*` carries a user-controlled URL, as do `\fig|src=` and the generic `link-href` / `link-id` attributes on character markers. A file containing `javascript:` or `data:text/html` is an XSS payload in a format users open without inspection.

```typescript
const SAFE_SCHEMES = new Set(["http:", "https:", "mailto:"]);

export function sanitizeHref(raw: string): SafeHref {
  // Internal scripture references: BOOK C:V — resolved in-app, never navigated.
  if (/^[A-Z1-9]{3}\s+\d+[:.]\d+/.test(raw.trim())) return { kind: "ref", value: raw.trim() };

  let u: URL;
  try { u = new URL(raw, "about:blank"); } catch { return { kind: "inert", value: raw }; }
  if (!SAFE_SCHEMES.has(u.protocol)) return { kind: "inert", value: raw };
  return { kind: "external", value: u.href };
}
```

- **`inert`** renders as plain text with a warning affordance, **not** an anchor — the user sees what the file contained without the application acting on it.
- **`ref`** resolves through Go to Reference and never navigates.
- **`external`** opens through the OS handler after a first-use confirmation, **never in the webview**. A link opening in the webview is a link running in the application's origin.

## 3. Figures

**Images are off by default**, with a per-document opt-in enabling local files only.

Remote schemes are never loaded — a placeholder renders instead, which prevents a document phoning home and leaking that a particular file was opened. Local paths resolve relative to the document's directory, rejecting `..` traversal and absolute paths, and the bytes are read **by the shell, for a document it currently holds open**, so access ends when the document does. 20 MB decode cap, checked before the file is read. The web build never loads local images.

> **On the asset protocol.** This section previously specified the Tauri asset protocol, scoped at runtime to the document's directory and dropped on close. The first half of that is available; the second is not. Tauri's filesystem scope is additive, and `forbid_directory` is documented to take precedence over allowed paths *always* — so the only way to withdraw a grant is a permanent denial, which does not drop access when the document closes, it poisons that directory for every document opened from it afterwards. A reader who closed a book and reopened it would find its figures gone for the rest of the session, with nothing to explain why.
>
> Reading through a command instead makes the lifetime exact rather than approximate: the grant *is* the open-document entry the shell already keeps, so there is no scope to remember to revoke. It also preserves this shell's stronger existing rule — no `fs:` permission of any kind, and every path the webview can reach is one a person chose in a native picker. The webview never learns the document's directory; it sends the path the `ig` asked for and the shell decides what that means. Implemented in `crates/easy-usfm-tauri/src/figure.rs`.

## 4. Content Security Policy

Applied to the Tauri webview and the hosted application alike:

```
default-src 'none';
script-src 'self' 'wasm-unsafe-eval';
style-src 'self';
img-src 'self' asset: http://asset.localhost;
font-src 'self';
connect-src 'self';
worker-src 'self' blob:;
base-uri 'none';
form-action 'none';
frame-ancestors 'none';
```

`'wasm-unsafe-eval'` is required to instantiate the engine and does not permit JavaScript `eval`. `img-src` permits the asset protocol while §3 gates whether anything is ever loaded through it — the CSP is the floor, not the policy.

**A known conflict, resolved in Phase 1.** Svelte's scoped styles are extracted at build time and satisfy `style-src 'self'`, but CodeMirror 6 injects its theme at runtime by creating a `<style>` element, which this policy blocks. A per-load nonce is impossible for a static build without an edge function; `'unsafe-inline'` is too weak. **Resolution: extract the CodeMirror theme to a static stylesheet at build time and disable runtime injection.** Build against the real CSP from the first commit — discovering this in Phase 3 means retrofitting theming across the whole editor surface.

## 5. Tauri capabilities

**The filesystem plugin is not exposed to the frontend at all** — there is no general `fs` scope to escape from. All access goes through purpose-built commands (`open_document`, `save_document`, `write_recovery`) that validate paths server-side. Dialog-granted paths enter a runtime scope dropped when the document closes.

A compromised frontend therefore cannot read arbitrary files even if every other control failed.

## 6. Logging and telemetry

**Logs** carry marker tags, diagnostic codes, offsets, file sizes, and timings. They **never** carry scripture text, paths outside the app data directory, or document content. "Export diagnostic bundle" is an explicit action that **displays exactly what will be included before writing it.**

**Telemetry is off.** Not configurable-to-on-by-default — off. No analytics, no crash reporting by default, no phone-home. Crashes write a **local** report with a stack trace, application and OS versions, and the non-content log records above, which the user may attach to an issue manually.

This is not only a privacy posture: the documents are frequently unpublished work under confidentiality obligations, and default-on collection would be disqualifying regardless of how carefully it was scrubbed.

**Network activity** is exactly one kind of request, ever: the update check, opt-in on first run with a prompt saying so ([PRODUCT §11](PRODUCT.md#11-platform)). A build variant with the updater compiled out exists for restricted deployments.

## 7. Release checklist

- [ ] CI grep for `{@html}` and `innerHTML` passes on `src/`.
- [ ] CSP present and identical in the Tauri config and the web build; no `unsafe-inline` in any directive.
- [ ] `sanitizeHref` tests cover `javascript:`, `data:`, `vbscript:`, `file:`, protocol-relative `//host`, and mixed-case scheme variants.
- [ ] A corpus file with a hostile `link-href` renders inert and navigates nowhere.
- [ ] Figures are off on a freshly opened document; the opt-in is per document and does not persist across files.
- [ ] `..` traversal in `\fig src=` is rejected, including URL-encoded forms.
- [ ] No Tauri command accepts a path without server-side validation.
- [ ] A diagnostic bundle from a real translation file contains no scripture text.
- [ ] Network capture during a full editing session shows zero requests with the updater disabled.
