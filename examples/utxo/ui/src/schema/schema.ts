import type {OpenAPIV3_1} from "openapi-types";

/**
 * Fetches and inlines a schema from an OpenAPI URL
 */
export async function fetchSchema(openapiUrl: string): Promise<OpenAPIV3_1.Document> {
    const res = await fetch(openapiUrl);
    if (!res.ok) throw new Error("Failed to fetch OpenAPI JSON");

    return (await res.json()) as OpenAPIV3_1.Document;
}
