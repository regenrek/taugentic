import type { CapsuleRecipe, RecipeListResponse } from "../contracts.js";
import { PROTOCOL_JSON_SCHEMAS } from "../../generated/runtime.js";
import { ajv, parseSchema } from "./core.js";

const validateCapsuleRecipe = ajv.compile(PROTOCOL_JSON_SCHEMAS.CapsuleRecipe);
const validateRecipeListResponse = ajv.compile(PROTOCOL_JSON_SCHEMAS.RecipeListResponse);

export function parseCapsuleRecipe(value: unknown): CapsuleRecipe {
  return parseSchema<CapsuleRecipe>("CapsuleRecipe", validateCapsuleRecipe, value);
}

export function parseRecipeListResponse(value: unknown): RecipeListResponse {
  return parseSchema<RecipeListResponse>("RecipeListResponse", validateRecipeListResponse, value);
}
