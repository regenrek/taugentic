import { describe, expect, it, vi } from "vite-plus/test";

import type { CapsuleRecipe } from "../../packages/shared/src/contracts.js";

type CreateElementFn = (
  component: unknown,
  props?: Record<string, unknown> | null,
  ...children: unknown[]
) => unknown;
type CreateRootFn = (container: Element) => {
  render: (element: unknown) => void;
  unmount: () => void;
};
type ActFn = (callback: () => void | Promise<void>) => Promise<void>;

const reactModulePath = "../../packages/renderer/node_modules/react/index.js";
const reactDomClientModulePath = "../../packages/renderer/node_modules/react-dom/client.js";
// @ts-expect-error jsdom ships without declarations in this workspace.
const { JSDOM } = (await import("jsdom")) as {
  JSDOM: new (
    html?: string,
    options?: { pretendToBeVisual?: boolean },
  ) => {
    window: Window & typeof globalThis;
  };
};

let currentAct: ActFn = async (callback) => {
  await callback();
};

describe("RecipePickerView", () => {
  it("renders an empty state when no recipes are registered", async () => {
    const rendered = await renderRecipePicker({ recipes: [] });
    const document = rendered.document;

    expect(document.body.textContent).toContain("No recipes registered.");
    expect(document.querySelector('[role="status"]')).not.toBeNull();
    rendered.unmount();
  });

  it("renders all built-in recipes with name and description", async () => {
    const rendered = await renderRecipePicker({ recipes: BUILTIN_RECIPES });
    const document = rendered.document;

    for (const recipe of BUILTIN_RECIPES) {
      expect(document.body.textContent).toContain(recipe.name);
      expect(document.body.textContent).toContain(recipe.description);
    }
    rendered.unmount();
  });

  it("opens the objective form when a recipe is clicked", async () => {
    const rendered = await renderRecipePicker({ recipes: BUILTIN_RECIPES });
    const document = rendered.document;
    const debugOption = optionByName(document, "Debug Agent");

    await currentAct(async () => {
      debugOption.click();
    });

    expect(document.querySelector('input[aria-label="Recipe objective"]')).not.toBeNull();
    expect(document.body.textContent).toContain("Debug Agent");
    rendered.unmount();
  });

  it("starts the selected recipe with objective and model override", async () => {
    const onStartRecipeRun = vi.fn(async () => {});
    const rendered = await renderRecipePicker({
      onStartRecipeRun,
      recipes: BUILTIN_RECIPES,
    });
    const document = rendered.document;

    await currentAct(async () => {
      optionByName(document, "Debug Agent").click();
    });
    await fillInput(document, "Recipe objective", "  Find login bug  ");
    await fillInput(document, "Override model", "  gpt-5.5-high  ");
    await currentAct(async () => {
      buttonByName(document, "start recipe").click();
    });

    expect(onStartRecipeRun).toHaveBeenCalledWith({
      modelId: "gpt-5.5-high",
      objective: "Find login bug",
      recipeId: "debug-agent",
    });
    rendered.unmount();
  });

  it("supports listbox keyboard navigation with arrow keys and enter", async () => {
    const rendered = await renderRecipePicker({ recipes: BUILTIN_RECIPES });
    const document = rendered.document;
    const listbox = document.querySelector('[role="listbox"]');

    await currentAct(async () => {
      listbox?.dispatchEvent(keyboardEvent("keydown", "ArrowDown"));
      listbox?.dispatchEvent(keyboardEvent("keydown", "Enter"));
    });

    expect(document.body.textContent).toContain("Patch Agent");
    expect(document.querySelector('input[aria-label="Recipe objective"]')).not.toBeNull();
    rendered.unmount();
  });

  it("exposes listbox and option roles for assistive technology", async () => {
    const rendered = await renderRecipePicker({ recipes: BUILTIN_RECIPES });
    const document = rendered.document;

    expect(document.querySelector('[role="listbox"]')).not.toBeNull();
    expect(document.querySelectorAll('[role="option"]')).toHaveLength(BUILTIN_RECIPES.length);
    rendered.unmount();
  });
});

async function renderRecipePicker({
  onStartRecipeRun = async () => {},
  recipes,
}: {
  onStartRecipeRun?: (input: unknown) => Promise<void> | void;
  recipes: CapsuleRecipe[];
}) {
  const dom = new JSDOM("<!doctype html><html><body><main></main></body></html>", {
    pretendToBeVisual: true,
  });
  installDomGlobals(dom);
  const { createElement } = (await import(reactModulePath)) as {
    createElement: CreateElementFn;
  };
  const { createRoot } = (await import(reactDomClientModulePath)) as {
    createRoot: CreateRootFn;
  };
  const { act } = (await import(reactModulePath)) as {
    act: ActFn;
  };
  const { RecipePickerPanel } =
    await import("../../packages/renderer/src/features/recipe-picker/RecipePicker.js");
  currentAct = act;
  const container = dom.window.document.querySelector("main");
  if (container === null) {
    throw new Error("test container missing");
  }
  const root = createRoot(container);
  await currentAct(async () => {
    root.render(
      createElement(RecipePickerPanel, {
        onStartRecipeRun,
        recipes,
      }),
    );
  });
  return {
    document: dom.window.document,
    unmount() {
      root.unmount();
      dom.window.close();
    },
  };
}

function installDomGlobals(dom: { window: Window & typeof globalThis }) {
  Object.assign(globalThis, {
    IS_REACT_ACT_ENVIRONMENT: true,
    document: dom.window.document,
    HTMLElement: dom.window.HTMLElement,
    KeyboardEvent: dom.window.KeyboardEvent,
    MouseEvent: dom.window.MouseEvent,
    Node: dom.window.Node,
    window: dom.window,
  });
}

function optionByName(document: Document, name: string): HTMLButtonElement {
  const match = Array.from(document.querySelectorAll<HTMLButtonElement>('[role="option"]')).find(
    (option) => option.textContent?.includes(name) ?? false,
  );
  if (!match) {
    throw new Error(`missing option ${name}`);
  }
  return match;
}

function buttonByName(document: Document, name: string): HTMLButtonElement {
  const match = Array.from(document.querySelectorAll<HTMLButtonElement>("button")).find(
    (button) => button.textContent?.includes(name) ?? false,
  );
  if (!match) {
    throw new Error(`missing button ${name}`);
  }
  return match;
}

async function fillInput(document: Document, label: string, value: string) {
  const input = document.querySelector<HTMLInputElement>(`input[aria-label="${label}"]`);
  if (!input) {
    throw new Error(`missing input ${label}`);
  }
  await currentAct(async () => {
    const view = input.ownerDocument.defaultView;
    Object.getOwnPropertyDescriptor(view?.HTMLInputElement.prototype, "value")?.set?.call(
      input,
      value,
    );
    input.dispatchEvent(new window.InputEvent("input", { bubbles: true, inputType: "insertText" }));
    input.dispatchEvent(new window.Event("change", { bubbles: true }));
  });
}

function keyboardEvent(type: string, key: string): KeyboardEvent {
  return new window.KeyboardEvent(type, { bubbles: true, key });
}

const BUILTIN_RECIPES: CapsuleRecipe[] = [
  makeRecipe("debug-agent", "Debug Agent", "Debugs a focused issue.", "debug"),
  makeRecipe("patch-agent", "Patch Agent", "Applies a focused change.", "patch"),
  makeRecipe("review-agent", "Review Agent", "Reviews a scoped diff.", "review"),
  makeRecipe("test-agent", "Test Agent", "Runs targeted verification.", "test"),
  makeRecipe("plan-agent", "Plan Agent", "Plans a multi-step task.", "plan"),
];

function makeRecipe(
  id: string,
  name: string,
  description: string,
  contract: CapsuleRecipe["contract"],
): CapsuleRecipe {
  return {
    contract,
    defaultModel: null,
    description,
    id,
    name,
    promptTemplate: `${name} prompt`,
  };
}
