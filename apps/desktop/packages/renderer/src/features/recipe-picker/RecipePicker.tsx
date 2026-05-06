import { useMemo, useState, type KeyboardEvent } from "react";

import type { AgentRuntimeModelId, CapsuleRecipe, SessionId } from "@taugentic/desktop-shared";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Popover } from "@/components/ui/popover";
import { useStartRunMutation } from "@/lib/queries/session-mutations";
import { useRecipesQuery } from "@/lib/queries/recipes";

export interface RecipePickerProps {
  sessionId: SessionId;
  onRunStarted?: () => void;
}

export interface StartRecipeRunInput {
  modelId?: AgentRuntimeModelId | null;
  objective: string;
  recipeId: string;
  sandboxProfile?: string | null;
}

export interface RecipePickerViewProps {
  defaultOpen?: boolean;
  errorMessage?: string | null;
  isLoading?: boolean;
  isSubmitting?: boolean;
  onStartRecipeRun: (input: StartRecipeRunInput) => Promise<void> | void;
  recipes: CapsuleRecipe[];
}

export function RecipePicker({ onRunStarted, sessionId }: RecipePickerProps) {
  const recipesQuery = useRecipesQuery();
  const startRun = useStartRunMutation(sessionId);

  async function startRecipeRun(input: StartRecipeRunInput) {
    await startRun.mutateAsync(input);
    onRunStarted?.();
  }

  return (
    <RecipePickerView
      errorMessage={recipesQuery.error ? toErrorMessage(recipesQuery.error) : null}
      isLoading={recipesQuery.isLoading}
      isSubmitting={startRun.isPending}
      onStartRecipeRun={startRecipeRun}
      recipes={recipesQuery.data ?? []}
    />
  );
}

function RecipePickerView({ defaultOpen = false, ...props }: RecipePickerViewProps) {
  return (
    <Popover.Root defaultOpen={defaultOpen}>
      <Popover.Trigger
        aria-label="Open recipe picker"
        className="inline-flex h-7 items-center rounded-[var(--radius)] border border-[var(--border)] px-2 font-[var(--font-mono)] text-[10px] uppercase tracking-[0.18em] text-[var(--fg-dim)] transition-colors hover:border-[var(--accent)] hover:text-[var(--fg)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--accent)]"
        type="button"
      >
        recipes
      </Popover.Trigger>
      <Popover.Content align="end" className="w-[24rem] p-0" side="top">
        <RecipePickerPanel {...props} />
      </Popover.Content>
    </Popover.Root>
  );
}

export function RecipePickerPanel({
  errorMessage,
  isLoading = false,
  isSubmitting = false,
  onStartRecipeRun,
  recipes,
}: Omit<RecipePickerViewProps, "defaultOpen">) {
  const [activeIndex, setActiveIndex] = useState(0);
  const [commandError, setCommandError] = useState<string | null>(null);
  const [modelId, setModelId] = useState("");
  const [objective, setObjective] = useState("");
  const [query, setQuery] = useState("");
  const [sandboxProfile, setSandboxProfile] = useState("");
  const [selectedRecipe, setSelectedRecipe] = useState<CapsuleRecipe | null>(null);

  const visibleRecipes = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (needle.length === 0) {
      return recipes;
    }
    return recipes.filter((recipe) =>
      [recipe.name, recipe.id, recipe.description ?? "", recipe.contract]
        .join(" ")
        .toLowerCase()
        .includes(needle),
    );
  }, [query, recipes]);
  const clampedActiveIndex =
    visibleRecipes.length === 0 ? 0 : Math.min(activeIndex, visibleRecipes.length - 1);

  function selectRecipe(recipe: CapsuleRecipe) {
    setCommandError(null);
    setModelId("");
    setObjective("");
    setSandboxProfile("");
    setSelectedRecipe(recipe);
  }

  function handleListKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (visibleRecipes.length === 0) {
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex((current) => (current + 1) % visibleRecipes.length);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex((current) => (current - 1 + visibleRecipes.length) % visibleRecipes.length);
    } else if (event.key === "Home") {
      event.preventDefault();
      setActiveIndex(0);
    } else if (event.key === "End") {
      event.preventDefault();
      setActiveIndex(visibleRecipes.length - 1);
    } else if (event.key === "Enter") {
      event.preventDefault();
      selectRecipe(visibleRecipes[clampedActiveIndex]);
    }
  }

  async function submitSelectedRecipe() {
    if (selectedRecipe === null || isSubmitting) {
      return;
    }
    const trimmedObjective = objective.trim();
    if (trimmedObjective.length === 0) {
      setCommandError("Objective is required.");
      return;
    }

    setCommandError(null);
    try {
      await onStartRecipeRun({
        modelId: optionalText(modelId) as AgentRuntimeModelId | null,
        objective: trimmedObjective,
        recipeId: selectedRecipe.id,
        sandboxProfile: optionalText(sandboxProfile),
      });
      setObjective("");
      setSelectedRecipe(null);
    } catch (error) {
      setCommandError(toErrorMessage(error));
    }
  }

  return (
    <div className="flex flex-col gap-2 p-3" data-recipe-picker="">
      <div>
        <div className="font-[var(--font-mono)] text-[11px] uppercase tracking-[0.18em] text-[var(--fg)]">
          Delegate recipe
        </div>
        <div className="text-[11px] text-[var(--fg-mute)]">
          Pick a daemon recipe, then provide the objective.
        </div>
      </div>
      <Input
        aria-label="Search recipes"
        className="h-7 border-[var(--border)] bg-[var(--bg)] px-2 font-[var(--font-mono)] text-[12px]"
        onChange={(event) => {
          setActiveIndex(0);
          setQuery(event.currentTarget.value);
        }}
        placeholder="search recipes..."
        value={query}
      />
      {isLoading ? (
        <div className="font-[var(--font-mono)] text-[11px] text-[var(--fg-mute)]">
          loading recipes...
        </div>
      ) : null}
      {errorMessage ? (
        <div className="font-[var(--font-mono)] text-[11px] text-[var(--status-failed)]">
          error: {errorMessage}
        </div>
      ) : null}
      {!isLoading && visibleRecipes.length === 0 ? (
        <div role="status" className="text-[12px] text-[var(--fg-mute)]">
          No recipes registered.
        </div>
      ) : (
        <div
          aria-label="Recipes"
          className="max-h-60 overflow-y-auto rounded-[var(--radius-sm)] border border-[var(--border)]"
          onKeyDown={handleListKeyDown}
          role="listbox"
          tabIndex={0}
        >
          {visibleRecipes.map((recipe, index) => (
            <button
              aria-selected={index === clampedActiveIndex}
              className="flex w-full flex-col gap-1 border-b border-[var(--border)] px-2 py-2 text-left last:border-b-0 hover:bg-[var(--surface-overlay)] aria-selected:bg-[var(--surface-overlay)]"
              key={recipe.id}
              onClick={() => selectRecipe(recipe)}
              role="option"
              type="button"
            >
              <span className="flex items-center gap-2">
                <span className="font-[var(--font-mono)] text-[12px] text-[var(--fg)]">
                  {recipe.name}
                </span>
                <span className="rounded-[var(--radius-sm)] border border-[var(--border)] px-1 font-[var(--font-mono)] text-[10px] uppercase tracking-[0.14em] text-[var(--fg-dim)]">
                  {recipe.contract}
                </span>
              </span>
              <span className="text-[11px] text-[var(--fg-mute)]">
                {recipe.description ?? recipe.id}
              </span>
            </button>
          ))}
        </div>
      )}
      {selectedRecipe !== null ? (
        <div className="flex flex-col gap-2 rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg)] p-2">
          <div className="font-[var(--font-mono)] text-[11px] text-[var(--fg)]">
            {selectedRecipe.name}
          </div>
          <Input
            aria-label="Recipe objective"
            className="h-7 border-[var(--border)] bg-[var(--bg)] px-2 font-[var(--font-mono)] text-[12px]"
            disabled={isSubmitting}
            onChange={(event) => setObjective(event.currentTarget.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                void submitSelectedRecipe();
              }
            }}
            placeholder="objective..."
            value={objective}
          />
          <div className="grid gap-2 sm:grid-cols-2">
            <Input
              aria-label="Override model"
              className="h-7 border-[var(--border)] bg-[var(--bg)] px-2 font-[var(--font-mono)] text-[12px]"
              disabled={isSubmitting}
              onChange={(event) => setModelId(event.currentTarget.value)}
              placeholder="model override"
              value={modelId}
            />
            <Input
              aria-label="Override sandbox"
              className="h-7 border-[var(--border)] bg-[var(--bg)] px-2 font-[var(--font-mono)] text-[12px]"
              disabled={isSubmitting}
              onChange={(event) => setSandboxProfile(event.currentTarget.value)}
              placeholder="sandbox override"
              value={sandboxProfile}
            />
          </div>
          {commandError !== null ? (
            <div className="font-[var(--font-mono)] text-[11px] text-[var(--status-failed)]">
              error: {commandError}
            </div>
          ) : null}
          <div className="flex justify-end gap-1">
            <Button
              disabled={isSubmitting}
              onClick={() => setSelectedRecipe(null)}
              size="sm"
              type="button"
              variant="ghost"
            >
              cancel
            </Button>
            <Button
              disabled={isSubmitting || objective.trim().length === 0}
              onClick={() => void submitSelectedRecipe()}
              size="sm"
              type="button"
              variant="secondary"
            >
              {isSubmitting ? "starting..." : "start recipe"}
            </Button>
          </div>
        </div>
      ) : null}
    </div>
  );
}

function optionalText(value: string): string | null {
  const trimmed = value.trim();
  return trimmed.length === 0 ? null : trimmed;
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
