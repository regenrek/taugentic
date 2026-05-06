import type { PropsWithChildren } from "react";

import { QueryClientProvider } from "@tanstack/react-query";

import { desktopQueryClient } from "./query-client";

export function AppProviders({ children }: PropsWithChildren) {
  return <QueryClientProvider client={desktopQueryClient}>{children}</QueryClientProvider>;
}
