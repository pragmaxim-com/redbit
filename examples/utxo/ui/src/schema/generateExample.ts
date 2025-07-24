import type { OpenAPIV3_1 } from "openapi-types";
import { COMPOSITES, isRef, resolveRef, SchemaMap, SchemaOrRef } from "./schema";

/** Generate multiple examples for a component */
export function generateExamples(root: string, defs: SchemaMap): any[] {
    const rootSchema = defs[root];
    return generateExamplesRec(rootSchema, defs);
}

/** Recursively generate all example values, especially for composites */
export function generateExamplesRec(val: SchemaOrRef, defs: SchemaMap): any[] {
    if (isRef(val)) {
        return generateExamplesRec(resolveRef(val.$ref, defs), defs);
    }

    const schema = val as OpenAPIV3_1.SchemaObject;

    // single example field
    if (schema.example !== undefined) {
        return [schema.example];
    }

    // multiple named examples
    if (Array.isArray((schema as any).examples) && (schema as any).examples.length) {
        return (schema as any).examples;
    }

    let results: any[] = [];

    // composite keywords: flatten examples from each branch
    for (const k of COMPOSITES) {
        const arr = (schema as any)[k];
        if (Array.isArray(arr)) {
            for (const sub of arr) {
                try {
                    results.push(...generateExamplesRec(sub, defs));
                } catch {
                    // skip invalid
                }
            }
            if (results.length) return results;
        }
    }

    // object: produce one example combining props
    if (schema.type === "object") {
        if (!schema.properties) return [{}];
        const objExamples: any = {};
        for (const [k, v] of Object.entries(schema.properties)) {
            const exs = generateExamplesRec(v as any, defs);
            if (exs.length) objExamples[k] = exs[0];
        }
        return [objExamples];
    }

    // array: produce one example array
    if (schema.type === "array") {
        if (!schema.items) return [[]];
        const itemExs = generateExamplesRec(schema.items as any, defs);
        return [itemExs];
    }

    // primitive fallback one example
    return [primitiveFallback(schema)];
}

function primitiveFallback(schema: OpenAPIV3_1.SchemaObject) {
    switch (schema.type) {
        case "string":
            if (schema.enum) return schema.enum[0];
            return "";
        case "number":
        case "integer":
            return typeof (schema as any).minimum === "number" ? (schema as any).minimum : 0;
        case "boolean":
            return false;
        default:
            return null;
    }
}
