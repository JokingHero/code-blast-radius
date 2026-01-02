import { invoke } from "@tauri-apps/api/core";
import { setIsLoading, setLoadingMessage } from "../store";

/**
 * Invokes a Tauri command with automatic global loading state management.
 * @param cmd The Rust command name
 * @param args Arguments for the command
 * @param message Optional text to display under the spinner (e.g. "INDEXING...")
 */
export async function invokeWithLoader<T>(
  cmd: string, 
  args?: Record<string, unknown>,
  message: string = "PROCESSING_DATA"
): Promise<T> {
  try {
    setLoadingMessage(message);
    setIsLoading(true);
    // Optional: minimal delay to prevent flicker on very fast operations
    // await new Promise(r => setTimeout(r, 200)); 
    return await invoke<T>(cmd, args);
  } catch (error) {
    console.error(`Error invoking ${cmd}:`, error);
    throw error;
  } finally {
    setIsLoading(false);
  }
}