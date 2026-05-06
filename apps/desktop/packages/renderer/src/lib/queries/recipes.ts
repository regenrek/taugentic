import { useQuery } from "@tanstack/react-query";

import type { CapsuleRecipe } from "@taugentic/desktop-shared";

import { listRecipes } from "@/lib/ipc/api";

import { queryKeys } from "./keys";
import { type SessionQueryView } from "./session-queries";

const RECIPE_STALE_TIME_MS = 10 * 60 * 1000;

export function useRecipesQuery(): SessionQueryView<CapsuleRecipe[]> {
  const query = useQuery({
    queryKey: queryKeys.recipes,
    queryFn: () => listRecipes(),
    select: (response) => response.recipes,
    staleTime: RECIPE_STALE_TIME_MS,
  });
  return {
    data: query.data,
    error: query.error,
    isLoading: query.isLoading,
    isFetching: query.isFetching,
    refetch: query.refetch,
  };
}
