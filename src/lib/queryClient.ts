import { QueryClient } from "@tanstack/react-query";

/** One client for the app, so stores can invalidate data after backend events. */
export const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 1, refetchOnWindowFocus: true } } });
