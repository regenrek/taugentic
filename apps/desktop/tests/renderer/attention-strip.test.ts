import { describe, expect, it } from "vite-plus/test";

import { AttentionStripView } from "../../packages/renderer/src/features/attention-strip/AttentionStrip.js";

type CreateElementFn = (component: unknown, props?: Record<string, unknown> | null) => unknown;
type RenderToStaticMarkupFn = (element: unknown) => string;

const reactModulePath = "../../packages/renderer/node_modules/react/index.js";
const reactServerModulePath = "../../packages/renderer/node_modules/react-dom/server.node.js";

const { createElement } = (await import(reactModulePath)) as {
  createElement: CreateElementFn;
};
const { renderToStaticMarkup } = (await import(reactServerModulePath)) as {
  renderToStaticMarkup: RenderToStaticMarkupFn;
};

function tagFor(markup: string, pillName: string): string {
  const needle = `data-attention-pill="${pillName}"`;
  const openTagEnd = markup.indexOf(needle);
  if (openTagEnd < 0) {
    throw new Error(`pill "${pillName}" not found in markup`);
  }
  const tagStart = markup.lastIndexOf("<", openTagEnd);
  const tagEnd = markup.indexOf(">", openTagEnd);
  if (tagStart < 0 || tagEnd < 0) {
    throw new Error(`tag boundaries not found for pill "${pillName}"`);
  }
  return markup.slice(tagStart, tagEnd + 1);
}

describe("AttentionStripView", () => {
  it("renders three pills with counts and tones from the snapshot", () => {
    const markup = renderToStaticMarkup(
      createElement(AttentionStripView, {
        state: {
          daemonHealthy: true,
          daemonLabel: "local · idle",
          failuresCount: 0,
          pendingApprovalsCount: 3,
          feedErrorMessage: null,
          feedHasLoaded: true,
        },
      }),
    );

    expect(markup).toContain('data-attention-pill="approvals"');
    expect(markup).toContain('data-attention-pill="failures"');
    expect(markup).toContain('data-attention-pill="daemon"');
    expect(markup).not.toContain('data-attention-pill="feed"');

    expect(tagFor(markup, "approvals")).toContain('data-tone="waiting"');
    expect(tagFor(markup, "failures")).toContain('data-tone="idle"');
    expect(tagFor(markup, "daemon")).toContain('data-tone="active"');

    expect(markup).toContain("APPROVALS");
    expect(markup).toContain("FAILURES");
    expect(markup).toContain("DAEMON");
    expect(markup).toContain(">3<");
    expect(markup).toContain("local · idle");
  });

  it("flips failures and daemon tone when counts rise and daemon is unhealthy", () => {
    const markup = renderToStaticMarkup(
      createElement(AttentionStripView, {
        state: {
          daemonHealthy: false,
          daemonLabel: "unavailable",
          failuresCount: 2,
          pendingApprovalsCount: 0,
          feedErrorMessage: null,
          feedHasLoaded: true,
        },
      }),
    );

    expect(tagFor(markup, "approvals")).toContain('data-tone="idle"');
    expect(tagFor(markup, "failures")).toContain('data-tone="failed"');
    expect(tagFor(markup, "daemon")).toContain('data-tone="failed"');
    expect(markup).toContain("unavailable");
  });

  it("renders a feed pill when the session overview read fails while daemon stays healthy", () => {
    const markup = renderToStaticMarkup(
      createElement(AttentionStripView, {
        state: {
          daemonHealthy: true,
          daemonLabel: "local · idle",
          failuresCount: 0,
          pendingApprovalsCount: 0,
          feedErrorMessage: "session overview timeout",
          feedHasLoaded: true,
        },
      }),
    );

    expect(tagFor(markup, "daemon")).toContain('data-tone="active"');
    expect(tagFor(markup, "feed")).toContain('data-tone="failed"');
    expect(markup).toContain("session overview timeout");
  });
});
