import Ajv2020, { type ErrorObject } from "ajv/dist/2020.js";

export const ajv = new Ajv2020({
  allErrors: true,
  formats: {
    uint32: true,
    uint64: true,
  },
  strict: true,
});
ajv.addFormat("uint32", {
  validate: (value: unknown) =>
    typeof value === "number" && Number.isInteger(value) && value >= 0 && value <= 4_294_967_295,
});
ajv.addFormat("uint64", {
  validate: (value: unknown) => typeof value === "number" && Number.isInteger(value) && value >= 0,
});

const PROTOCOL_UINT64_MAX = 18_446_744_073_709_551_615n;
const PROTOCOL_UINT64_MAX_DIGITS = PROTOCOL_UINT64_MAX.toString().length;

export class ProtocolValidationError extends Error {
  constructor(message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = "ProtocolValidationError";
  }
}

export function parseSchema<T>(
  schemaName: string,
  validator: ((value: unknown) => boolean) & { errors?: ErrorObject[] | null },
  value: unknown,
): T {
  if (validator(value)) {
    return value as T;
  }

  throw new ProtocolValidationError(formatProtocolValidationErrors(schemaName, validator.errors));
}

export function parseNullableBoundaryValue<Input, Result>(
  value: Input | null | undefined,
  parse: (value: Input) => Result,
): Result | null {
  return value == null ? null : parse(value);
}

export function parseNonEmptyProtocolString(value: string, fieldName: string): string {
  if (value.trim().length === 0) {
    throw new ProtocolValidationError(`${fieldName} must be a non-empty string`);
  }

  return value;
}

export function parseProtocolBigInt(value: string, fieldName: string): bigint {
  if (!/^\d+$/.test(value)) {
    throw new ProtocolValidationError(`${fieldName} must be a uint64 decimal string`);
  }
  if (value.length > PROTOCOL_UINT64_MAX_DIGITS) {
    throw new ProtocolValidationError(`${fieldName} must be <= ${PROTOCOL_UINT64_MAX.toString()}`);
  }

  const parsed = BigInt(value);
  if (parsed > PROTOCOL_UINT64_MAX) {
    throw new ProtocolValidationError(`${fieldName} must be <= ${PROTOCOL_UINT64_MAX.toString()}`);
  }

  return parsed;
}

export function parseStringField(value: unknown, fieldName: string): string {
  if (typeof value !== "string") {
    throw new ProtocolValidationError(`${fieldName} must be a string`);
  }
  return value;
}

export function formatProtocolValidationErrors(
  schemaName: string,
  errors: ErrorObject[] | null | undefined,
): string {
  if (!errors || errors.length === 0) {
    return `${schemaName} failed protocol validation`;
  }

  return `${schemaName} failed protocol validation: ${errors
    .map((error) => `${error.instancePath || "/"} ${error.message ?? "invalid"}`.trim())
    .join("; ")}`;
}
