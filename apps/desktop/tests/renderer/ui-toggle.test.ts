import { describe, expect, it } from "vite-plus/test";

import { Toggle } from "../../packages/renderer/src/components/ui/toggle.js";

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

const TOGGLE_VARIANTS = ["default", "outline", "ghost"] as const;
const TOGGLE_SIZES = ["sm", "md", "icon"] as const;

describe("Toggle", () => {
  for (const variant of TOGGLE_VARIANTS) {
    for (const size of TOGGLE_SIZES) {
      it(`renders variant=${variant} size=${size} with stable data attributes`, () => {
        const markup = renderToStaticMarkup(
          createElement(Toggle, { variant, size }, `tg-${variant}-${size}`),
        );
        expect(markup).toContain(`data-variant="${variant}"`);
        expect(markup).toContain(`data-size="${size}"`);
        expect(markup).toContain(`tg-${variant}-${size}`);
        expect(markup.startsWith("<button")).toBe(true);
      });
    }
  }

  it("defaults to variant=default size=md when unspecified", () => {
    const markup = renderToStaticMarkup(createElement(Toggle, null, "x"));
    expect(markup).toContain('data-variant="default"');
    expect(markup).toContain('data-size="md"');
  });

  it("renders aria-pressed=true when defaultPressed is true", () => {
    const markup = renderToStaticMarkup(createElement(Toggle, { defaultPressed: true }, "on"));
    expect(markup).toContain('aria-pressed="true"');
    expect(markup).toContain("on");
  });

  it("renders aria-pressed=false by default", () => {
    const markup = renderToStaticMarkup(createElement(Toggle, null, "off"));
    expect(markup).toContain('aria-pressed="false"');
  });
});
