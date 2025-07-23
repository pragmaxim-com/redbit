import type { OpenAPIV3_1 } from "openapi-types";
import {COMPOSITES, isRef, resolveRef, SchemaMap, SchemaOrRef} from "./schema";

export function generateExample(root: string, defs: SchemaMap): any {
    const rootSchema = defs[root];
    return generateExampleRec(rootSchema, defs)
}

export function generateExampleRec(val: SchemaOrRef, defs: SchemaMap): any {
    if (isRef(val)) return generateExampleRec(resolveRef(val.$ref, defs), defs);

    const schema = val as OpenAPIV3_1.SchemaObject;
    if (schema.example !== undefined) return schema.example;
    if (Array.isArray((schema as any).examples)) return (schema as any).examples![0];

    // try composites
    for (const k of COMPOSITES) {
        const arr = (schema as any)[k];
        if (Array.isArray(arr)) {
            for (const sub of arr) {
                try { return generateExampleRec(sub, defs); }
                catch { /* try next */ }
            }
            break;
        }
    }

    // object
    if (schema.type === "object") {
        if (!schema.properties) return {};
        const out: any = {};
        for (const [k, v] of Object.entries(schema.properties)) {
            out[k] = generateExampleRec(v, defs);
        }
        return out;
    }

    // array
    if (schema.type === "array") {
        if (!schema.items) return [];
        return [generateExampleRec(schema.items as any, defs)];
    }

    // primitive fallback
    return primitiveFallback(schema);
}

function primitiveFallback(schema: OpenAPIV3_1.SchemaObject) {
    switch (schema.type) {
        case "string":
            if (schema.enum) return schema.enum[0];
            return "";
        case "number": case "integer":
            return typeof schema.minimum === "number" ? schema.minimum : 0;
        case "boolean":
            return false;
        default:
            return null;
    }
}

