export interface AddFilesParams {
  pattern: string;
}

export interface RemoveFilesParams {
  pattern: string;
}

export interface BlastRadiusParams {
  symbol: string;
  max_depth: number;
  exclude_tests: boolean;
}

// Define the Union matched to #[serde(tag = "type", content = "params")]
export type RecipeOperation =
  | { type: "AddFiles"; params: AddFilesParams }
  | { type: "RemoveFiles"; params: RemoveFilesParams }
  | { type: "BlastRadius"; params: BlastRadiusParams };

export type RecipeOperationType = RecipeOperation["type"];

export type FileTransformMode =
  | { mode: "Skeletonize"; symbols: string[] }
  | { mode: "FocusOn"; symbols: string[] };

export interface EngineRecipe {
  name: string;
  description: string | null;
  operations: RecipeOperation[];
  transforms: Record<string, FileTransformMode>;
  default_transform: FileTransformMode | null;
}

export interface UiRecipeStep {
  id: string;
  op: RecipeOperation;
}

export interface SavedRecipe extends EngineRecipe {}