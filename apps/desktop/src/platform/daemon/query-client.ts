import { QueryClient } from "@tanstack/react-query"

/** The only TypeScript cache for daemon-owned desktop projections. */
export const desktopQueryClient = new QueryClient({
  defaultOptions: { queries: { retry: false, staleTime: Infinity } },
})
