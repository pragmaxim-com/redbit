// inlinePaths.ts
import type { OpenAPIV3_1 } from "openapi-types";
import { inlineValueRefs, SchemaMap } from "./inlineSchema";

// --- Types ---
export type InlineSchema = OpenAPIV3_1.SchemaObject;

export interface ParamInfo {
    name: string;
    in: "path" | "query" | "header" | "cookie";
    required: boolean;
    /** name of the referenced component if original schema was a $ref */
    refName?: string;
    example?: any;
    schema: InlineSchema;
}

export interface Endpoint {
    operationId: string;
    methodName: string;
    title: string;
    method: string;
    path: string;
    params: ParamInfo[];
    requestBody?: { schema: InlineSchema; refName?: string; example?: any; mediaType: string };
    responseSchemas: Record<string, InlineSchema>;
    responseMediaTypes: Record<string, string>;
    streaming: boolean;
    tags: string[];
}

export type EndpointMap = Record<string, Endpoint>;

// --- Helpers ---
function pickMedia(content: Record<string, any>) {
    const keys = Object.keys(content);
    const jsonKey = keys.find(k => /json$/i.test(k));
    const key = jsonKey || keys[0];
    const entry = content[key];
    return entry && { mediaType: key, schema: entry.schema, example: entry.example };
}

function toCamel(s: string): string {
    return s.replace(/_([a-z])/g, (_, c) => c.toUpperCase());
}

function buildParams(params: (OpenAPIV3_1.ReferenceObject | OpenAPIV3_1.ParameterObject)[], defs: SchemaMap): ParamInfo[] {
    return params.map(p => {
        const param = p as OpenAPIV3_1.ParameterObject;
        const rawSchema = (param as any).schema;
        const refName = rawSchema?.$ref?.split('/').pop();
        const schema = inlineValueRefs(rawSchema || {}, defs) as InlineSchema;
        return {
            name: param.name,
            in: param.in as any,
            required: Boolean(param.required),
            refName,
            example: (param as any).example,
            schema,
        };
    });
}

function buildRequestBody(rb: OpenAPIV3_1.RequestBodyObject | undefined, defs: SchemaMap, opId: string) {
    if (!rb) return undefined;
    const media = pickMedia(rb.content as any);
    if (!media) return undefined;
    const refName = media.schema.$ref?.split('/').pop();
    const schema = inlineValueRefs(media.schema, defs) as InlineSchema;
    return { schema, refName, example: media.example, mediaType: media.mediaType };
}

function buildResponses(responses: OpenAPIV3_1.ResponsesObject | undefined, defs: SchemaMap) {
    const schemas: Record<string, InlineSchema> = {};
    const mediaTypes: Record<string, string> = {};
    let streaming = false;
    for (const [code, resp] of Object.entries(responses || {})) {
        const r = resp as OpenAPIV3_1.ResponseObject;
        if (!r.content) continue;
        const media = pickMedia(r.content as any);
        if (!media) continue;
        const schema = inlineValueRefs(media.schema, defs) as InlineSchema;
        schemas[code] = schema;
        mediaTypes[code] = media.mediaType;
        if (media.mediaType === 'application/x-ndjson') streaming = true;
    }
    return { schemas, mediaTypes, streaming };
}

function buildEndpoint(
    path: string,
    method: string,
    op: OpenAPIV3_1.OperationObject,
    defs: SchemaMap
): Endpoint | undefined {
    if (!op?.operationId) return undefined;

    const operationId = op.operationId;
    const methodName = toCamel(operationId);
    const title = op.summary || op.description || operationId;
    const tags = op.tags || [];

    const params = buildParams(op.parameters || [], defs);
    const requestBody = buildRequestBody(op.requestBody as OpenAPIV3_1.RequestBodyObject, defs, operationId);
    const { schemas: responseSchemas, mediaTypes: responseMediaTypes, streaming } = buildResponses(op.responses, defs);

    return {
        operationId,
        methodName,
        title,
        method: method.toUpperCase(),
        path,
        params,
        requestBody,
        responseSchemas,
        responseMediaTypes,
        streaming,
        tags,
    };
}

export function generateEndpoints(raw: OpenAPIV3_1.PathsObject, defs: SchemaMap): EndpointMap {
    const map: EndpointMap = {};

    for (const [path, pathItem] of Object.entries(raw)) {
        if (!pathItem) continue;

        Object.entries(pathItem).forEach(([method, opObj]) => {
            const ep = buildEndpoint(path, method, opObj as OpenAPIV3_1.OperationObject, defs);
            if (ep) map[ep.operationId] = ep;
        });
    }

    return map;
}
