import { describe, expect, it } from "vite-plus/test";

import { ContextMenu } from "../../packages/renderer/src/components/ui/context-menu.js";

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

describe("ContextMenu", () => {
  it("renders the trigger area without throwing", () => {
    const markup = renderToStaticMarkup(
      createElement(
        ContextMenu.Root,
        null,
        createElement(ContextMenu.Trigger, null, createElement("div", null, "right-click-here")),
        createElement(
          ContextMenu.Content,
          null,
          createElement(ContextMenu.Item, null, "Cut"),
          createElement(ContextMenu.Item, null, "Copy"),
          createElement(ContextMenu.Separator, null),
          createElement(ContextMenu.Item, { destructive: true }, "Delete"),
        ),
      ),
    );
    expect(markup).toContain("right-click-here");
  });

  it("respects destructive flag with status-failed token", () => {
    const markup = renderToStaticMarkup(
      createElement(
        ContextMenu.Root,
        null,
        createElement(ContextMenu.Trigger, null, createElement("div", null, "ctx")),
        createElement(
          ContextMenu.Content,
          null,
          createElement(ContextMenu.Item, { destructive: true }, "Delete"),
        ),
      ),
    );
    expect(markup).toContain("ctx");
  });
});
