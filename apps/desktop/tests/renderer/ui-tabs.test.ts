import { describe, expect, it } from "vite-plus/test";

import { Tabs } from "../../packages/renderer/src/components/ui/tabs.js";

type CreateElementFn = (
  component: unknown,
  props?: Record<string, unknown> | null,
  ...children: unknown[]
) => unknown;
type RenderToStaticMarkupFn = (element: unknown) => string;

const reactModulePath = "../../packages/renderer/node_modules/react/index.js";
const reactServerModulePath = "../../packages/renderer/node_modules/react-dom/server.node.js";

const { createElement } = (await import(reactModulePath)) as {
  createElement: CreateElementFn;
};
const { renderToStaticMarkup } = (await import(reactServerModulePath)) as {
  renderToStaticMarkup: RenderToStaticMarkupFn;
};

function renderTabs(value: string, listVariant?: "default" | "line"): string {
  return renderToStaticMarkup(
    createElement(
      Tabs.Root,
      { value },
      createElement(
        Tabs.List,
        listVariant ? { variant: listVariant } : null,
        createElement(Tabs.Trigger, { value: "overview" }, "Overview"),
        createElement(Tabs.Trigger, { value: "logs" }, "Logs"),
      ),
      createElement(Tabs.Content, { value: "overview" }, "overview-content"),
      createElement(Tabs.Content, { value: "logs" }, "logs-content"),
    ),
  );
}

describe("Tabs", () => {
  it("renders triggers with role=tab and the active panel as role=tabpanel", () => {
    const markup = renderTabs("overview");
    expect(markup).toContain("Overview");
    expect(markup).toContain("Logs");
    expect(markup).toContain('role="tab"');
    expect(markup).toContain('role="tablist"');
    expect(markup).toContain('role="tabpanel"');
    expect(markup).toContain("overview-content");
  });

  it("marks the matching trigger as aria-selected=true", () => {
    const markup = renderTabs("logs");
    const aria = markup.match(/aria-selected="true"/g) ?? [];
    expect(aria.length).toBe(1);
    const logsButtonIdx = markup.indexOf("Logs");
    const ariaTrueIdx = markup.indexOf('aria-selected="true"');
    expect(ariaTrueIdx).toBeGreaterThan(-1);
    expect(ariaTrueIdx).toBeLessThan(logsButtonIdx);
  });

  it("List variant=default vs variant=line emits different data-variant", () => {
    const def = renderTabs("overview", "default");
    const line = renderTabs("overview", "line");
    expect(def).toContain('data-variant="default"');
    expect(line).toContain('data-variant="line"');
    expect(line).toContain("border-b");
  });
});
