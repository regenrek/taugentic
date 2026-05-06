import { describe, expect, it } from "vite-plus/test";

import { Badge } from "../../packages/renderer/src/components/ui/badge.js";
import { Button } from "../../packages/renderer/src/components/ui/button.js";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "../../packages/renderer/src/components/ui/card.js";
import { Dialog } from "../../packages/renderer/src/components/ui/dialog.js";
import { Input } from "../../packages/renderer/src/components/ui/input.js";
import { Popover } from "../../packages/renderer/src/components/ui/popover.js";
import { Separator } from "../../packages/renderer/src/components/ui/separator.js";
import { StatusDot } from "../../packages/renderer/src/components/ui/status-dot.js";
import { Tooltip } from "../../packages/renderer/src/components/ui/tooltip.js";

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

const BUTTON_VARIANTS = ["default", "secondary", "outline", "destructive", "ghost"] as const;
const BUTTON_SIZES = ["sm", "default", "lg", "icon"] as const;
const BADGE_VARIANTS = ["default", "secondary", "outline", "destructive", "accent"] as const;
const STATUS_TONES = ["active", "waiting", "failed", "completed", "cancelled", "idle"] as const;

describe("Button", () => {
  for (const variant of BUTTON_VARIANTS) {
    for (const size of BUTTON_SIZES) {
      it(`renders variant=${variant} size=${size} with stable data attributes`, () => {
        const markup = renderToStaticMarkup(
          createElement(Button, { variant, size }, `btn-${variant}-${size}`),
        );
        expect(markup).toContain(`data-variant="${variant}"`);
        expect(markup).toContain(`data-size="${size}"`);
        expect(markup).toContain(`btn-${variant}-${size}`);
        expect(markup.startsWith("<button")).toBe(true);
      });
    }
  }

  it("defaults to variant=default size=default when unspecified", () => {
    const markup = renderToStaticMarkup(createElement(Button, null, "ok"));
    expect(markup).toContain('data-variant="default"');
    expect(markup).toContain('data-size="default"');
  });

  it("honors asChild by rendering provided child element", () => {
    const markup = renderToStaticMarkup(
      createElement(
        Button,
        { asChild: true, variant: "ghost" },
        createElement("a", { href: "#go" }, "go"),
      ),
    );
    expect(markup.startsWith("<a")).toBe(true);
    expect(markup).toContain('href="#go"');
    expect(markup).toContain('data-variant="ghost"');
  });
});

describe("Badge", () => {
  for (const variant of BADGE_VARIANTS) {
    it(`renders variant=${variant}`, () => {
      const markup = renderToStaticMarkup(createElement(Badge, { variant }, `badge-${variant}`));
      expect(markup).toContain(`data-variant="${variant}"`);
      expect(markup).toContain(`badge-${variant}`);
    });
  }

  it("uses compact terminal typography", () => {
    const markup = renderToStaticMarkup(createElement(Badge, null, "x"));
    expect(markup).toContain("text-[10px]");
    expect(markup).toContain("uppercase");
    expect(markup).toContain("tracking-[0.18em]");
  });
});

describe("Card family", () => {
  it("renders all card slots", () => {
    const markup = renderToStaticMarkup(
      createElement(
        Card,
        null,
        createElement(
          CardHeader,
          null,
          createElement(CardTitle, null, "title"),
          createElement(CardDescription, null, "desc"),
        ),
        createElement(CardContent, null, "content"),
        createElement(CardFooter, null, "footer"),
      ),
    );
    expect(markup).toContain("title");
    expect(markup).toContain("desc");
    expect(markup).toContain("content");
    expect(markup).toContain("footer");
    expect(markup).toContain("rounded-[var(--radius)]");
    expect(markup).toContain("border-[var(--border)]");
  });
});

describe("Input", () => {
  it("renders a text input with terminal tokens", () => {
    const markup = renderToStaticMarkup(createElement(Input, { placeholder: "enter" }));
    expect(markup).toContain('placeholder="enter"');
    expect(markup).toContain("rounded-[var(--radius)]");
    expect(markup).toContain("border-[var(--border)]");
  });
});

describe("Separator", () => {
  it("renders horizontal by default", () => {
    const markup = renderToStaticMarkup(createElement(Separator, null));
    expect(markup).toContain("bg-[var(--border)]");
    expect(markup).toContain("h-px");
  });

  it("renders vertical when orientation=vertical", () => {
    const markup = renderToStaticMarkup(createElement(Separator, { orientation: "vertical" }));
    expect(markup).toContain("w-px");
  });
});

describe("StatusDot", () => {
  for (const tone of STATUS_TONES) {
    it(`renders tone=${tone} with var(--status-${tone}) background`, () => {
      const markup = renderToStaticMarkup(createElement(StatusDot, { tone }));
      expect(markup).toContain(`data-tone="${tone}"`);
      expect(markup).toContain(`background-color:var(--status-${tone})`);
      expect(markup).toContain('aria-hidden="true"');
    });
  }

  it("includes the label text when provided and keeps the dot decorative", () => {
    const markup = renderToStaticMarkup(
      createElement(StatusDot, { tone: "active", label: "Online" }),
    );
    expect(markup).toContain("Online");
    expect(markup).toContain('aria-hidden="true"');
    expect(markup).toContain("background-color:var(--status-active)");
  });
});

describe("Overlay primitives (smoke render)", () => {
  it("renders Tooltip root + trigger without throwing", () => {
    const markup = renderToStaticMarkup(
      createElement(
        Tooltip.Provider,
        null,
        createElement(
          Tooltip.Root,
          null,
          createElement(Tooltip.Trigger, null, "hover"),
          createElement(Tooltip.Content, null, "tip"),
        ),
      ),
    );
    expect(markup).toContain("hover");
  });

  it("renders Popover root + trigger without throwing", () => {
    const markup = renderToStaticMarkup(
      createElement(
        Popover.Root,
        null,
        createElement(Popover.Trigger, null, "open"),
        createElement(Popover.Content, null, "body"),
      ),
    );
    expect(markup).toContain("open");
  });

  it("renders Dialog root + trigger without throwing", () => {
    const markup = renderToStaticMarkup(
      createElement(
        Dialog.Root,
        null,
        createElement(Dialog.Trigger, null, "launch"),
        createElement(
          Dialog.Content,
          null,
          createElement(Dialog.Title, null, "hello"),
          createElement(Dialog.Description, null, "world"),
        ),
      ),
    );
    expect(markup).toContain("launch");
  });
});
