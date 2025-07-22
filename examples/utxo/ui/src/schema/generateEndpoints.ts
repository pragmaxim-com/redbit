import type { OpenAPIV3_1 } from "openapi-types";
import { inlineValueRefs, SchemaMap } from "./inlineSchema";
import { generateExample } from "./generateExample";

export type InlineSchema = OpenAPIV3_1.SchemaObject;

export interface ParamDefinition {
    name: string;
    in: "path" | "query" | "header" | "cookie";
    required: boolean;
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
    paramDefs: ParamDefinition[];
    exampleParams: Record<string, any>[];
    requestBody?: { schema: InlineSchema; refName?: string; example?: any; mediaType: string };
    responseSchemas: Record<string, InlineSchema>;
    responseMediaTypes: Record<string, string>;
    streaming: boolean;
    tags: string[];
}

export type EndpointMap = Record<string, Endpoint>;

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

function buildParamDefinitions(
    params: (OpenAPIV3_1.ReferenceObject | OpenAPIV3_1.ParameterObject)[],
    defs: SchemaMap
): ParamDefinition[] {
    return params
        .filter(p => !("$ref" in p))
        .map(p => {
            const param = p as OpenAPIV3_1.ParameterObject;
            const rawSchema = (param as any).schema || {};
            const refName = rawSchema.$ref?.split('/').pop();
            const schema = inlineValueRefs(rawSchema, defs) as InlineSchema;
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

function buildRequestBody(
    rb: OpenAPIV3_1.RequestBodyObject | undefined,
    defs: SchemaMap
) {
    if (!rb) return undefined;
    const media = pickMedia(rb.content as any);
    if (!media) return undefined;
    const schema = inlineValueRefs(media.schema, defs) as InlineSchema;
    const refName = media.schema.$ref?.split('/').pop();
    return { schema, refName, example: media.example, mediaType: media.mediaType };
}

function buildResponses(
    responses: OpenAPIV3_1.ResponsesObject | undefined,
    defs: SchemaMap
) {
    const responseSchemas: Record<string, InlineSchema> = {};
    const responseMediaTypes: Record<string, string> = {};
    let streaming = false;
    for (const [code, resp] of Object.entries(responses || {})) {
        const r = resp as OpenAPIV3_1.ResponseObject;
        if (!r.content) continue;
        const media = pickMedia(r.content as any);
        if (!media) continue;
        responseSchemas[code] = inlineValueRefs(media.schema, defs) as InlineSchema;
        responseMediaTypes[code] = media.mediaType;
        if (/ndjson$/i.test(media.mediaType)) streaming = true;
    }
    return { responseSchemas, responseMediaTypes, streaming };
}

function buildExampleEndpointParams(
    paramDefs: ParamDefinition[],
    requestBody: Endpoint['requestBody'],
    defs: SchemaMap,
    streaming: boolean
): Record<string, any>[] {
    const required = paramDefs.filter(p => p.required);
    const optional = paramDefs.filter(p => !p.required);
    const variants: ParamDefinition[][] = [
        [...required, ...optional],
        ...optional.map(p => [...required, p])
    ];
    return variants.map(paramsList => {
        const args: any = streaming ? { throwOnError: false, parseAs: 'stream' } : { throwOnError: false };
        paramsList.forEach(p => {
            args[p.in] = args[p.in] || {};
            args[p.in][p.name] = p.example !== undefined
                ? p.example
                : generateExample(p.refName || p.name, defs);
        });
        if (requestBody) {
            const key = requestBody.refName || '';
            args.body = requestBody.example !== undefined
                ? requestBody.example
                : generateExample(key, defs);
        }
        return args;
    });
}

function buildEndpoint(
    path: string,
    method: string,
    op: OpenAPIV3_1.OperationObject,
    defs: SchemaMap
): Endpoint {
    const operationId = op.operationId!;
    const methodName = toCamel(operationId);
    const title = op.summary || op.description || operationId;
    const tags = op.tags || [];

    const paramDefs = buildParamDefinitions(op.parameters || [], defs);
    const requestBody = buildRequestBody(op.requestBody as OpenAPIV3_1.RequestBodyObject, defs);
    const { responseSchemas, responseMediaTypes, streaming } = buildResponses(op.responses, defs);
    const exampleParams = buildExampleEndpointParams(paramDefs, requestBody, defs, streaming);

    return {
        operationId,
        methodName,
        title,
        method: method.toUpperCase(),
        path,
        paramDefs,
        exampleParams,
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
            map[ep.operationId] = ep;
        });
    }
    return map;
}
