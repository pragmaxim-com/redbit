import type { OpenAPIV3_1 } from "openapi-types";

type SchemaObjectOrRef = OpenAPIV3_1.SchemaObject | OpenAPIV3_1.ReferenceObject;
export type SchemaMap = Record<string, SchemaObjectOrRef>;

export function resolveRef(ref: string, defs: SchemaMap): SchemaObjectOrRef | undefined {
    const match = ref.match(/^#\/components\/schemas\/(.+)$/);
    if (!match) return undefined;
    return defs[match[1]];
}

/**
 * Recursively inlines $refs and nested schemas
 */
export function inlineValueRefs(val: SchemaObjectOrRef, defs: SchemaMap): SchemaObjectOrRef {
    if (Array.isArray(val)) {
        return val.map((v) => inlineValueRefs(v, defs)) as any;
    }

    if (typeof val === "object" && val !== null) {
        if ("$ref" in val) {
            const resolved = resolveRef(val.$ref, defs);
            if (!resolved) throw new Error(`Unresolved $ref: ${val.$ref}`);
            return inlineSchemaRec(resolved, defs);
        }

        // At this point, val is a SchemaObject, not a ReferenceObject
        const newVal: OpenAPIV3_1.SchemaObject = { ...val };

        for (const keyword of ["oneOf", "anyOf", "allOf"] as const) {
            if (Array.isArray(newVal[keyword])) {
                newVal[keyword] = newVal[keyword]!.map((sub) =>
                    inlineValueRefs(sub, defs)
                );
            }
        }

        if (newVal.properties) {
            const inlinedProps: Record<string, SchemaObjectOrRef> = {};
            for (const [key, prop] of Object.entries(newVal.properties)) {
                inlinedProps[key] = inlineValueRefs(prop, defs);
            }
            newVal.properties = inlinedProps;
        }

        if (newVal.type === "array" && typeof newVal.items !== "undefined") {
            newVal.items = inlineValueRefs(newVal.items, defs);
        }
        return newVal;
    }

    return val;
}

export function inlineSchema(root: string, defs: SchemaMap): SchemaObjectOrRef {
    const rootSchema = defs[root];
    return inlineSchemaRec(rootSchema, defs)
}

function inlineSchemaRec(schema: SchemaObjectOrRef, defs: SchemaMap): SchemaObjectOrRef {
    const cloned = JSON.parse(JSON.stringify(schema)) as SchemaObjectOrRef;
    return inlineValueRefs(cloned, defs);
}
