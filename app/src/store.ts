import { createSignal } from "solid-js";

export const [isLoading, setIsLoading] = createSignal(false);
export const [loadingMessage, setLoadingMessage] = createSignal("PROCESSING");