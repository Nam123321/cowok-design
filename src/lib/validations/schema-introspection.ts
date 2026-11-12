/**
 * schema-introspection.ts — Runtime Zod Schema Introspection & Boundary Verification
 *
 * Implements Layer 1 (L1) and Layer 3 (L3) checks for ADR-091 Khóa C1 / R1.
 * Pure, deterministic walker over Zod schema definitions (_def).
 *
 * Enforces:
 * - [EC-P11-001] Whitelist of known safe Zod typeNames (default-deny for unrecognized types).
 * - [EC-P11-002] Explicit exclusion of ZodCustom, ZodAny, ZodUnknown, ZodNever.
 * - [EC-P1-002] Recursive unwrapping of all container/wrapper types.
 * - [EC-P1-005] Recursive unwrapping of compound types (Union, Intersection, Effects, Pipeline, Lazy).
 * - [EC-P4-001] String matching on _def.typeName instead of instanceof.
 * - [EC-P1-004] Safe structural checking before property access.
 * - Non-empty ZodObject: Object.keys(shape).length > 0.
 * - Cycle detection via WeakSet + max depth guard (default 50).
 */

export interface IntrospectionResult {
  valid: boolean;
  reason?: string;
  path?: string;
}

export interface IntrospectionOptions {
  allowUnknown?: boolean;
  maxDepth?: number;
}

const SAFE_TERMINAL_TYPES = new Set([
  'ZodString',
  'ZodNumber',
  'ZodNaN',
  'ZodBigInt',
  'ZodBoolean',
  'ZodDate',
  'ZodSymbol',
  'ZodUndefined',
  'ZodNull',
  'ZodVoid',
  'ZodLiteral',
  'ZodEnum',
  'ZodNativeEnum',
]);

const BANNED_TYPES = new Set([
  'ZodAny',
  'ZodUnknown',
  'ZodNever',
  'ZodCustom',
]);

const KNOWN_CONTAINER_TYPES = new Set([
  'ZodArray',
  'ZodObject',
  'ZodUnion',
  'ZodDiscriminatedUnion',
  'ZodIntersection',
  'ZodTuple',
  'ZodRecord',
  'ZodMap',
  'ZodSet',
  'ZodLazy',
  'ZodEffects',
  'ZodOptional',
  'ZodNullable',
  'ZodDefault',
  'ZodCatch',
  'ZodPromise',
  'ZodBranded',
  'ZodPipeline',
  'ZodReadonly',
]);

export function introspectSchema(
  schema: unknown,
  options: IntrospectionOptions = {},
  currentPath = '$',
  depth = 0,
  visited = new WeakSet<object>()
): IntrospectionResult {
  const maxDepth = options.maxDepth ?? 50;

  if (depth > maxDepth) {
    return {
      valid: false,
      reason: `Maximum schema inspection depth (${maxDepth}) exceeded (potential circular reference or extreme nesting)`,
      path: currentPath,
    };
  }

  // [EC-P1-004] Type-checking crash prevention on non-objects / null / undefined
  if (!schema || typeof schema !== 'object' || !('_def' in schema)) {
    return {
      valid: false,
      reason: `Target is not a valid Zod schema object (missing _def)`,
      path: currentPath,
    };
  }

  // Cycle guard
  if (visited.has(schema)) {
    return { valid: true };
  }
  visited.add(schema);

  const def = (schema as { _def?: any })._def;
  if (!def || typeof def !== 'object') {
    return {
      valid: false,
      reason: `Schema _def property is invalid`,
      path: currentPath,
    };
  }

  // [EC-P4-001] String matching on _def.typeName
  const typeName = def.typeName;
  if (typeof typeName !== 'string') {
    return {
      valid: false,
      reason: `Schema _def.typeName is missing or not a string`,
      path: currentPath,
    };
  }

  // [EC-P11-002] Explicitly deny banned types
  if (BANNED_TYPES.has(typeName)) {
    if (typeName === 'ZodUnknown' && options.allowUnknown) {
      return { valid: true };
    }
    return {
      valid: false,
      reason: `Forbidden no-op or escape-hatch schema type '${typeName}'`,
      path: currentPath,
    };
  }

  // Safe terminal types pass directly
  if (SAFE_TERMINAL_TYPES.has(typeName)) {
    return { valid: true };
  }

  // [EC-P11-001] Explicit whitelist: if it is neither safe terminal nor known container, FAIL-CLOSED
  if (!KNOWN_CONTAINER_TYPES.has(typeName)) {
    return {
      valid: false,
      reason: `Unrecognized or non-whitelisted schema type '${typeName}'`,
      path: currentPath,
    };
  }

  // [EC-P1-002] & [EC-P1-005] Recursive unwrapping of containers and compounds
  switch (typeName) {
    case 'ZodObject': {
      const shape = typeof def.shape === 'function' ? def.shape() : def.shape;
      if (!shape || typeof shape !== 'object') {
        return {
          valid: false,
          reason: `ZodObject shape is invalid or cannot be inspected`,
          path: currentPath,
        };
      }
      const keys = Object.keys(shape);
      if (keys.length === 0) {
        return {
          valid: false,
          reason: `ZodObject has an empty shape ({}) which provides zero validation`,
          path: currentPath,
        };
      }
      for (const k of keys) {
        const res = introspectSchema(shape[k], options, `${currentPath}.${k}`, depth + 1, visited);
        if (!res.valid) return res;
      }
      return { valid: true };
    }

    case 'ZodArray':
      return introspectSchema(def.type, options, `${currentPath}[]`, depth + 1, visited);

    case 'ZodOptional':
    case 'ZodNullable':
    case 'ZodDefault':
    case 'ZodCatch':
    case 'ZodReadonly':
      return introspectSchema(def.innerType, options, `${currentPath}*`, depth + 1, visited);

    case 'ZodPromise':
    case 'ZodBranded':
      return introspectSchema(def.type, options, currentPath, depth + 1, visited);

    case 'ZodSet':
      return introspectSchema(def.valueType, options, `${currentPath}<Set>`, depth + 1, visited);

    case 'ZodMap': {
      const keyRes = introspectSchema(def.keyType, options, `${currentPath}<MapKey>`, depth + 1, visited);
      if (!keyRes.valid) return keyRes;
      return introspectSchema(def.valueType, options, `${currentPath}<MapVal>`, depth + 1, visited);
    }

    case 'ZodRecord': {
      if (def.keyType) {
        const keyRes = introspectSchema(def.keyType, options, `${currentPath}<RecordKey>`, depth + 1, visited);
        if (!keyRes.valid) return keyRes;
      }
      return introspectSchema(def.valueType, options, `${currentPath}<RecordVal>`, depth + 1, visited);
    }

    case 'ZodTuple': {
      const items = Array.isArray(def.items) ? def.items : [];
      for (let i = 0; i < items.length; i++) {
        const res = introspectSchema(items[i], options, `${currentPath}[${i}]`, depth + 1, visited);
        if (!res.valid) return res;
      }
      if (def.rest) {
        return introspectSchema(def.rest, options, `${currentPath}[...rest]`, depth + 1, visited);
      }
      return { valid: true };
    }

    case 'ZodUnion':
    case 'ZodDiscriminatedUnion': {
      const rawOptions = def.options instanceof Map ? Array.from(def.options.values()) : def.options;
      const optsList = Array.isArray(rawOptions) ? rawOptions : [];
      if (optsList.length === 0) {
        return {
          valid: false,
          reason: `${typeName} has no branch options`,
          path: currentPath,
        };
      }
      for (let i = 0; i < optsList.length; i++) {
        const res = introspectSchema(optsList[i], options, `${currentPath}|branch[${i}]`, depth + 1, visited);
        if (!res.valid) {
          return {
            valid: false,
            reason: `${typeName} branch at index ${i} is invalid: ${res.reason}`,
            path: res.path,
          };
        }
      }
      return { valid: true };
    }

    case 'ZodIntersection': {
      const leftRes = introspectSchema(def.left, options, `${currentPath}&left`, depth + 1, visited);
      if (!leftRes.valid) return leftRes;
      return introspectSchema(def.right, options, `${currentPath}&right`, depth + 1, visited);
    }

    case 'ZodEffects':
      return introspectSchema(def.schema, options, `${currentPath}#effects`, depth + 1, visited);

    case 'ZodPipeline': {
      const inRes = introspectSchema(def.in, options, `${currentPath}#pipelineIn`, depth + 1, visited);
      if (!inRes.valid) return inRes;
      return introspectSchema(def.out, options, `${currentPath}#pipelineOut`, depth + 1, visited);
    }

    case 'ZodLazy': {
      try {
        const inner = typeof def.getter === 'function' ? def.getter() : undefined;
        if (!inner) {
          return {
            valid: false,
            reason: `ZodLazy getter did not return a valid schema`,
            path: currentPath,
          };
        }
        return introspectSchema(inner, options, `${currentPath}#lazy`, depth + 1, visited);
      } catch (err: any) {
        return {
          valid: false,
          reason: `ZodLazy getter execution threw an error: ${err?.message || err}`,
          path: currentPath,
        };
      }
    }

    default:
      return {
        valid: false,
        reason: `Unhandled whitelisted container type '${typeName}'`,
        path: currentPath,
      };
  }
}

/**
 * Asserts that the schema is meaningful and safe. Throws a TypeError on failure.
 */
export function assertMeaningfulSchema(schema: unknown, options?: IntrospectionOptions): void {
  const result = introspectSchema(schema, options);
  if (!result.valid) {
    throw new TypeError(`[Zod Introspection Failure] ${result.reason} at path: '${result.path}'`);
  }
}
