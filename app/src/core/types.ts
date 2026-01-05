export type RecipeOperationType = "AddFiles" | "RemoveFiles" | "BlastRadius";
export interface RecipeOperation {
  type: RecipeOperationType;
  params: {
    pattern?: string; // For AddFiles/RemoveFiles
    symbol?: string; // For BlastRadius
    max_depth?: number; // For BlastRadius
    exclude_tests?: boolean; // For BlastRadius
  };
}
export type FileTransformMode =
  | { mode: "Skeletonize"; symbols: string[] } // Hide specific symbols
  | { mode: "FocusOn"; symbols: string[] }; // Hide everything EXCEPT these
export interface EngineRecipe {
  name: string;
  description: string | null;
  operations: RecipeOperation[];
  transforms: Record<string, FileTransformMode>; // Specific file overrides
  default_transform: FileTransformMode | null; // Fallback (null = Full Text)
}
// UI Representation of a Step (with a unique ID for drag-drop/reactivity)
export interface UiRecipeStep {
  id: string;
  op: RecipeOperation;
}
export interface SavedRecipe extends EngineRecipe {
  // Matches Rust struct exactly
}
