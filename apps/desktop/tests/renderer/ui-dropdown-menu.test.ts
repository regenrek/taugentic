import { describe, expect, it } from "vite-plus/test";

import { DropdownMenu } from "../../packages/renderer/src/components/ui/dropdown-menu.js";

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

describe("DropdownMenu", () => {
  it("renders trigger with aria-haspopup=menu and the trigger label", () => {
    const markup = renderToStaticMarkup(
      createElement(
        DropdownMenu.Root,
        null,
        createElement(DropdownMenu.Trigger, null, "open-menu"),
        createElement(
          DropdownMenu.Content,
          null,
          createElement(DropdownMenu.Item, null, "Save"),
          createElement(DropdownMenu.Item, { destructive: true }, "Delete"),
        ),
      ),
    );
    expect(markup).toContain("open-menu");
    expect(markup).toContain('aria-haspopup="menu"');
    expect(markup.startsWith("<button")).toBe(true);
  });

  it("does not crash and exposes interactive trigger when toggled controlled-open", () => {
    const markup = renderToStaticMarkup(
      createElement(
        DropdownMenu.Root,
        { open: true, modal: false },
        createElement(DropdownMenu.Trigger, null, "trigger"),
        createElement(
          DropdownMenu.Content,
          null,
          createElement(DropdownMenu.Label, null, "Actions"),
          createElement(DropdownMenu.Item, null, "Save"),
          createElement(DropdownMenu.Separator, null),
          createElement(DropdownMenu.Item, { destructive: true }, "Delete"),
        ),
      ),
    );
    expect(markup).toContain("trigger");
    expect(markup).toContain('aria-haspopup="menu"');
  });
});
