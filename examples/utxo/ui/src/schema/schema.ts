import type {OpenAPIV3_1} from "openapi-types";
import {inlineValueRefs} from "./inlineSchema";
import {generateExampleRec} from "./generateExample";

export type SchemaOrRef = OpenAPIV3_1.SchemaObject | OpenAPIV3_1.ReferenceObject;
export type SchemaMap = Record<string, SchemaOrRef>;

export const COMPOSITES = ["oneOf","anyOf","allOf"] as const;
/**
 * Fetches and inlines a schema from an OpenAPI URL
 */
export async function fetchSchema(openapiUrl: string): Promise<OpenAPIV3_1.Document> {
    const res = await fetch(openapiUrl);
    if (!res.ok) throw new Error("Failed to fetch OpenAPI JSON");

    return (await res.json()) as OpenAPIV3_1.Document;
}


// resolve a $ref
export function resolveRef(ref: string, defs: SchemaMap): SchemaOrRef {
    const [, name] = ref.match(/^#\/components\/schemas\/(.+)$/) || [];
    if (!name || !defs[name]) throw new Error(`Unresolved $ref ${ref}`);
    return defs[name];
}


export function isRef(s: any): s is OpenAPIV3_1.ReferenceObject {
    return typeof s === "object" && s !== null && "$ref" in s;
}

/**
 * Inline a schema and annotate it with an "examples" array containing a single example value
 */
export function inlineSchemaWithExample(val: SchemaOrRef, defs: SchemaMap, example: any): OpenAPIV3_1.SchemaObject {
    const schema = inlineValueRefs(val, defs);
    schema.examples = [example || generateExampleRec(schema, defs)];
    return schema;
}