type CreateElementFn = (
  component: unknown,
  props?: Record<string, unknown> | null,
  ...children: unknown[]
) => unknown;

interface QueryClientCtor {
  new (config?: unknown): unknown;
}

interface QueryModule {
  QueryClient: QueryClientCtor;
  QueryClientProvider: unknown;
}

const reactModulePath = "../../../packages/renderer/node_modules/react/index.js";
const queryModulePath =
  "../../../packages/renderer/node_modules/@tanstack/react-query/build/modern/index.js";

const { createElement } = (await import(reactModulePath)) as {
  createElement: CreateElementFn;
};
const { QueryClient, QueryClientProvider } = (await import(queryModulePath)) as QueryModule;

export function createTestQueryClient(): unknown {
  return new QueryClient({
    defaultOptions: {
      queries: {
        gcTime: 0,
        staleTime: 0,
        retry: false,
        refetchOnWindowFocus: false,
        refetchOnReconnect: false,
        refetchOnMount: false,
      },
      mutations: {
        retry: false,
      },
    },
  });
}

export function withQueryClient(client: unknown, node: unknown): unknown {
  return createElement(QueryClientProvider, { client }, node);
}
