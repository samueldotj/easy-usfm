/**
 * URL sanitization — P3.7.
 *
 * SECURITY §2's threat is concrete: a `javascript:` or `data:text/html` URL in
 * a `\jmp` link, in a file that arrived by email and is opened without
 * inspection. So the tests are the payloads, not the happy path.
 */

import { describe, expect, it } from "vitest";

import { isLocalFigurePath, sanitizeHref } from "./href";

const kind = (raw: string) => sanitizeHref(raw).kind;

describe("links that must never be navigated", () => {
  it("makes script URLs inert", () => {
    expect(kind("javascript:alert(1)")).toBe("inert");
    expect(kind("JavaScript:alert(1)")).toBe("inert");
    // Whitespace and control characters inside the scheme are the classic
    // way past a naive prefix check.
    expect(kind("  javascript:alert(1)")).toBe("inert");
    expect(kind("java\tscript:alert(1)")).toBe("inert");
  });

  it("makes data and blob URLs inert", () => {
    // `data:text/html` is a document with an origin, which is the whole point.
    expect(kind("data:text/html,<script>alert(1)</script>")).toBe("inert");
    expect(kind("blob:https://x.test/abc")).toBe("inert");
  });

  it("makes file URLs inert", () => {
    expect(kind("file:///etc/passwd")).toBe("inert");
  });

  it("makes a protocol-relative URL inert rather than resolving it", () => {
    // Against the application's own origin this would become a real remote
    // URL. Parsed against `about:blank`, it has nowhere to go.
    expect(kind("//evil.test/payload")).toBe("inert");
  });

  it("makes a relative path inert", () => {
    expect(kind("/etc/passwd")).toBe("inert");
    expect(kind("../../secret")).toBe("inert");
  });

  it("keeps the original text so the user can see what the file said", () => {
    const payload = "javascript:alert(1)";
    expect(sanitizeHref(payload)).toEqual({ kind: "inert", value: payload });
  });
});

describe("links that may be opened", () => {
  it("allows the three schemes SECURITY §2 names", () => {
    expect(kind("https://example.test/page")).toBe("external");
    expect(kind("http://example.test/page")).toBe("external");
    expect(kind("mailto:someone@example.test")).toBe("external");
  });

  it("normalizes what it lets through", () => {
    const safe = sanitizeHref("https://example.test/a/../b");
    expect(safe).toEqual({ kind: "external", value: "https://example.test/b" });
  });
});

describe("scripture references", () => {
  it("recognises them before trying to parse a URL", () => {
    // Without this they would come back inert, and every internal reference
    // in every file would look like a security warning.
    expect(sanitizeHref("GEN 1:1")).toEqual({ kind: "ref", value: "GEN 1:1" });
    expect(kind("MAT 5.3")).toBe("ref");
  });

  it("recognises book codes that begin with a digit", () => {
    // 1CO, 2SA, 3JN — a third of the New Testament.
    expect(kind("1CO 13:4")).toBe("ref");
    expect(kind("3JN 1:2")).toBe("ref");
  });

  it("does not mistake a URL for one", () => {
    expect(kind("https://example.test/GEN 1:1")).toBe("external");
  });
});

describe("figure paths", () => {
  it("accepts an ordinary relative path", () => {
    expect(isLocalFigurePath("images/map.png")).toBe(true);
    expect(isLocalFigurePath("map.png")).toBe(true);
  });

  it("rejects traversal, including encoded forms", () => {
    // SECURITY §3 names encoded traversal specifically. A check that only
    // sees the raw form is a check that has been walked around.
    expect(isLocalFigurePath("../secrets.png")).toBe(false);
    expect(isLocalFigurePath("images/../../secrets.png")).toBe(false);
    expect(isLocalFigurePath("%2e%2e%2fsecrets.png")).toBe(false);
    expect(isLocalFigurePath("%252e%252e%252fsecrets.png")).toBe(false);
    // Windows separators, which a `/`-only check walks straight through.
    expect(isLocalFigurePath("..\\..\\secrets.png")).toBe(false);
  });

  it("rejects absolute paths", () => {
    expect(isLocalFigurePath("/etc/passwd")).toBe(false);
    expect(isLocalFigurePath("C:\\Windows\\win.ini")).toBe(false);
  });

  it("rejects anything remote, so a document cannot phone home", () => {
    expect(isLocalFigurePath("https://tracker.test/pixel.png")).toBe(false);
    expect(isLocalFigurePath("ftp://host/x.png")).toBe(false);
  });

  it("rejects malformed encoding rather than guessing", () => {
    expect(isLocalFigurePath("%zz.png")).toBe(false);
  });

  it("rejects an empty path", () => {
    expect(isLocalFigurePath("")).toBe(false);
    expect(isLocalFigurePath("   ")).toBe(false);
  });
});
