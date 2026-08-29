import { queryOptions } from "@tanstack/react-query"

import type { RecipeListResponse } from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "./desktop-runtime.js"

export const recipesQueryKey = ["daemon", "recipes"] as const

/** The only desktop cache for the daemon-owned recipe registry projection. */
export function recipesQuery(runtime: Pick<DesktopRuntime, "listRecipes">) {
  return queryOptions({
    queryKey: recipesQueryKey,
    queryFn: (): Promise<RecipeListResponse> => runtime.listRecipes(),
  })
}
