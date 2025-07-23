// inlineSchema.ts
import type { OpenAPIV3_1 } from "openapi-types";

type SchemaObjectOrRef = OpenAPIV3_1.SchemaObject | OpenAPIV3_1.ReferenceObject;
export type SchemaMap = Record<string, SchemaObjectOrRef>;

// --- Core visitor ---
type VisitAction<T> = {
    onRef?: (ref: string) => T;
    onObject?: (obj: OpenAPIV3_1.SchemaObject, children: Record<string, T>, listChildren: T[]) => T;
    onPrimitive?: (schema: OpenAPIV3_1.SchemaObject) => T;
};

function visitSchema<T>(
    schema: SchemaObjectOrRef,
    defs: SchemaMap,
    action: VisitAction<T>,
    path = "#"
): T {
    // resolve $ref
    if (typeof schema === "object" && schema !== null && "$ref" in schema) {
        if (!action.onRef) throw new Error(`Unexpected $ref at ${path}`);
        const match = schema.$ref.match(/^#\/components\/schemas\/(.+)$/);
        const resolved = match ? defs[match[1]] : undefined;
        if (!resolved) throw new Error(`Unresolved $ref ${schema.$ref} at ${path}`);
        return action.onRef(schema.$ref);
    }

    // At this point it's a SchemaObject
    const obj = schema as OpenAPIV3_1.SchemaObject;

    // collect children by property
    const propChildren: Record<string, T> = {};
    if (obj.properties) {
        for (const [key, sub] of Object.entries(obj.properties)) {
            propChildren[key] = visitSchema(sub, defs, action, `${path}.properties.${key}`);
        }
    }

    // collect children by composite keywords
    const listChildren: T[] = [];
    for (const kw of ["oneOf", "anyOf", "allOf"] as const) {
        if (Array.isArray(obj[kw])) {
            obj[kw]!.forEach((sub, i) => {
                listChildren.push(visitSchema(sub, defs, action, `${path}.${kw}[${i}]`));
            });
        }
    }

    // array items
    if (obj.type === "array" && obj.items) {
        listChildren.push(visitSchema(obj.items, defs, action, `${path}.items`));
    }

    // primitive or object
    if (action.onObject) {
        return action.onObject(obj, propChildren, listChildren);
    } else if (action.onPrimitive) {
        return action.onPrimitive(obj);
    } else {
        throw new Error(`No visitor for schema at ${path}`);
    }
}

// --- inlineValueRefs ---
export function inlineValueRefs(val: SchemaObjectOrRef, defs: SchemaMap): SchemaObjectOrRef {
    return visitSchema<SchemaObjectOrRef>(val, defs, {
        onRef: (ref) => {
            const name = ref.split('/').pop()!;
            const resolved = defs[name];
            if (!resolved) throw new Error(`Unresolved $ref: ${ref}`);
            return inlineValueRefs(resolved, defs);
        },
        onObject: (obj, props, list) => {
            if (obj.type === 'array') {
                // ArraySchemaObject
                const arr: OpenAPIV3_1.ArraySchemaObject = { ...obj, items: list.length ? list[list.length - 1] : obj.items! };
                if (Object.keys(props).length) arr.properties = props as any;
                return arr;
            } else {
                const out: OpenAPIV3_1.SchemaObject = { ...obj };
                if (Object.keys(props).length) out.properties = props as any;
                return out;
            }
        },
        onPrimitive: schema => schema,
    });
}

// --- generateExampleRec ---
export function generateExampleRec(val: SchemaObjectOrRef, defs: SchemaMap): any {
    return visitSchema<any>(val, defs, {
        onRef: (ref) => {
            const name = ref.split('/').pop()!;
            const resolved = defs[name];
            if (!resolved) throw new Error(`Unresolved $ref: ${ref}`);
            return generateExampleRec(resolved, defs);
        },
        onObject: (obj, propEx, listEx) => {
            if (obj.example !== undefined) return obj.example;
            if (Array.isArray((obj as any).examples)) return (obj as any).examples[0];
            if (obj.type === 'array') {
                // arrays: wrap first child in array
                return [listEx[0]];
            }
            if (listEx.length) return listEx[0];
            if (obj.type === 'object') return propEx;
            return primitiveFallback(obj);
        },
        onPrimitive: schema => primitiveFallback(schema),
    });
}

// --- public entrypoints ---
function inlineSchema(root: string, defs: SchemaMap): SchemaObjectOrRef {
    const rootSchema = defs[root];
    if (!rootSchema) throw new Error(`Schema ${root} not found`);
    return inlineValueRefs(rootSchema, defs);
}

export function generateExample(root: string, defs: SchemaMap): any {
    const rootSchema = defs[root];
    if (!rootSchema) throw new Error(`Schema ${root} not found`);
    return generateExampleRec(rootSchema, defs);
}

function primitiveFallback(schema: OpenAPIV3_1.SchemaObject) {
    const types = Array.isArray(schema.type) ? schema.type : [schema.type];
    const nonNull = types.filter(t => t && t !== 'null');
    const t = nonNull[0] || 'null';
    switch (t) {
        case 'string': return '';
        case 'number': return 0;
        case 'integer': return 0;
        case 'boolean': return false;
        case 'null': return null;
        case 'array': return [];
        case 'object': return {};
        default: return null;
    }
}
