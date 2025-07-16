import type { OpenAPIV3_1 } from "openapi-types";
import { resolveRef } from "./inlineSchema";

type SchemaObjectOrRef = OpenAPIV3_1.SchemaObject | OpenAPIV3_1.ReferenceObject;
type SchemaMap = Record<string, SchemaObjectOrRef>;

function isReferenceObject(schema: unknown): schema is OpenAPIV3_1.ReferenceObject {
    return typeof schema === 'object' && schema !== null && '$ref' in schema;
}

export function generateExample(root: string, defs: SchemaMap): any {
    const rootSchema = defs[root];
    return generateExampleRec(rootSchema, defs)
}

export function generateExampleRec(
    schema: SchemaObjectOrRef,
    defs: SchemaMap,
    path: string = ''
): any {
    if (isReferenceObject(schema)) {
        const resolved = resolveRef(schema.$ref, defs);
        if (!resolved) throw new Error(`Unresolved $ref: ${schema.$ref} at ${path}`);
        return generateExampleRec(resolved, defs, path);
    }

    if (schema.example !== undefined) {
        return schema.example;
    }

    if (Array.isArray((schema as any).examples) && (schema as any).examples.length > 0) {
        return (schema as any).examples[0];
    }

    // Handle oneOf/anyOf/allOf
    for (const discriminator of ["oneOf", "anyOf", "allOf"] as const) {
        const variants = schema[discriminator];
        if (Array.isArray(variants)) {
            for (const variant of variants) {
                try {
                    return generateExampleRec(variant, defs, `${path}.${discriminator}[i]`);
                } catch (e) {
                    // try next
                }
            }
            throw new Error(`No valid example found in ${discriminator} at ${path}`);
        }
    }

    if (schema.type === 'object') {
        if (!schema.properties) {
            throw new Error(`Missing .properties and .example at ${path}`);
        }

        const result: Record<string, any> = {};
        for (const [key, propSchema] of Object.entries(schema.properties)) {
            result[key] = generateExampleRec(propSchema, defs, `${path}.${key}`);
        }
        return result;
    }

    if (schema.type === 'array') {
        if (!schema.items) {
            throw new Error(`Missing .items and .example at ${path}`);
        }
        return [generateExampleRec(schema.items, defs, `${path}[]`)];
    }

    return getPrimitiveFallbackExample(schema, path);
}

function getPrimitiveFallbackExample(
    schema: OpenAPIV3_1.SchemaObject,
    path: string
): string | number | boolean {
    switch (schema.type) {
        case 'string':
            return 'string';
        case 'integer':
        case 'number':
            return 0;
        case 'boolean':
            return false;
        default:
            throw new Error(`No example for schema at ${path} with unknown primitive type`);
    }
}
