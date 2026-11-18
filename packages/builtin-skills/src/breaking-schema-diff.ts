// @story 08.3
// @implements AC-03, AC-05

/** Deterministic key-order-independent JSON equality check matching Rust Option omitted-key semantics. */
export function stableJsonEqual(a: unknown, b: unknown): boolean {
  const norm = (v: unknown): unknown => {
    if (Array.isArray(v)) return v.map((item) => (item === undefined ? null : norm(item)));
    if (v && typeof v === "object") {
      const obj = v as Record<string, unknown>;
      return Object.fromEntries(
        Object.keys(obj).sort().filter((k) => obj[k] !== undefined).map((k) => [k, norm(obj[k])])
      );
    }
    return v ?? null;
  };
  return JSON.stringify(norm(a)) === JSON.stringify(norm(b));
}

const asRecord = (val: unknown): Record<string, unknown> | null =>
  val !== null && typeof val === "object" && !Array.isArray(val) ? (val as Record<string, unknown>) : null;

function canonicalTypeKey(schemaVal: unknown): string {
  const rawType = asRecord(schemaVal)?.type;
  if (rawType === undefined) return "__NONE__";
  if (typeof rawType === "string") return `SET:${JSON.stringify([rawType])}`;
  if (Array.isArray(rawType) && rawType.every((x) => typeof x === "string")) {
    return `SET:${JSON.stringify([...new Set(rawType)].sort())}`;
  }
  return `RAW:${JSON.stringify(rawType)}`;
}

function diffConstraintsNode(pathPrefix: string, curObj: Record<string, unknown>, candObj: Record<string, unknown>, reasons: string[]): void {
  const curReq = new Set(Array.isArray(curObj.required) ? curObj.required.filter((v): v is string => typeof v === "string") : []);
  const candReq = new Set(Array.isArray(candObj.required) ? candObj.required.filter((v): v is string => typeof v === "string") : []);
  for (const added of candReq) if (!curReq.has(added)) reasons.push(`Added new required parameter '${pathPrefix}${added}'`);
  for (const removed of curReq) if (!candReq.has(removed)) reasons.push(`Removed previously required parameter '${pathPrefix}${removed}'`);
  const loc = pathPrefix || "<root>";
  if (curObj.additionalProperties !== false && candObj.additionalProperties === false) reasons.push(`Tightened additionalProperties=false at '${loc}'`);
  for (const kw of ["$ref", "const", "pattern", "format"] as const) {
    if (Object.hasOwn(curObj, kw) && curObj[kw] !== undefined) {
      if (!Object.hasOwn(candObj, kw) || candObj[kw] === undefined || !stableJsonEqual(curObj[kw], candObj[kw])) {
        reasons.push(`Schema keyword '${kw}' changed at '${loc}'`);
      }
    } else if (kw !== "$ref" && Object.hasOwn(candObj, kw) && candObj[kw] !== undefined) {
      reasons.push(`Added tightening constraint '${kw}' at '${loc}'`);
    }
  }
  if (Array.isArray(candObj.enum)) {
    const curEnums = Array.isArray(curObj.enum) ? curObj.enum : null;
    const candEnums = candObj.enum as unknown[];
    if (!curEnums || curEnums.some((v) => !candEnums.some((nv) => stableJsonEqual(v, nv)))) {
      reasons.push(`Narrowed or added enum constraint at '${loc}'`);
    }
  }
  for (const comb of ["oneOf", "anyOf", "allOf"] as const) {
    if (curObj[comb] !== undefined) {
      if (!stableJsonEqual(curObj[comb], candObj[comb])) {
        reasons.push(`Changed '${comb}' combinator schema at '${loc}'`);
      }
    } else if (candObj[comb] !== undefined) {
      reasons.push(`Added '${comb}' combinator constraint at '${loc}'`);
    }
  }
}

function diffSchemaNode(pathPrefix: string, curObj: Record<string, unknown>, candObj: Record<string, unknown>, reasons: string[]): void {
  diffConstraintsNode(pathPrefix, curObj, candObj, reasons);
  const curProps = asRecord(curObj.properties);
  const candProps = asRecord(candObj.properties);
  if (curProps && candProps) {
    for (const [propName, oldVal] of Object.entries(curProps)) {
      const nextVal = candProps[propName];
      if (nextVal === undefined) {
        reasons.push(`Removed parameter property '${pathPrefix}${propName}'`);
      } else if (canonicalTypeKey(oldVal) !== canonicalTypeKey(nextVal)) {
        reasons.push(`Parameter '${pathPrefix}${propName}' type changed`);
      } else {
        const oldRec = asRecord(oldVal);
        const newRec = asRecord(nextVal);
        if (oldRec && newRec) diffSchemaNode(`${pathPrefix}${propName}.`, oldRec, newRec, reasons);
      }
    }
  } else if (curProps && !candProps) {
    reasons.push(`Removed parameter 'properties' definition on '${pathPrefix}'`);
  }
  const curItems = asRecord(curObj.items);
  const candItems = asRecord(candObj.items);
  if (curItems && candItems) {
    if (canonicalTypeKey(curItems) !== canonicalTypeKey(candItems)) reasons.push(`Array items type changed on '${pathPrefix}items'`);
    else diffSchemaNode(`${pathPrefix}items.`, curItems, candItems, reasons);
  }
}

export function diffParameterSchemas(current: unknown, candidate: unknown, reasons: string[]): void {
  const curNull = current === null || current === undefined;
  const candNull = candidate === null || candidate === undefined;
  if (curNull || candNull) {
    if (!curNull && candNull) reasons.push("Parameter schema removed completely");
    else if (curNull && !candNull) {
      const candObj = asRecord(candidate);
      if (candObj && Array.isArray(candObj.required) && candObj.required.length > 0) reasons.push("Added new required parameters without existing schema");
      else if (!candObj) reasons.push("Parameter schema structural type changed (expected JSON object)");
    }
    return;
  }
  const curObj = asRecord(current);
  const candObj = asRecord(candidate);
  if (!curObj || !candObj) {
    if (!stableJsonEqual(current, candidate)) reasons.push("Parameter schema structural type changed");
    return;
  }
  diffSchemaNode("", curObj, candObj, reasons);
}
