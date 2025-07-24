import type { OpenAPIV3_1 } from "openapi-types";
import {inlineSchemaWithExample, SchemaMap} from "./schema";

export type InlinedSchema = OpenAPIV3_1.SchemaObject;

export interface ParamDefinition {
    name: string;
    in: "path" | "query" | "header" | "cookie";
    required: boolean;
    schema: InlinedSchema;
}

interface RequestBody {
    schema: InlinedSchema;
    required: boolean;
    mediaType: string;
}

interface ResponseBody {
    schema: InlinedSchema;
    mediaType: string;
    streaming: boolean;
}

export interface Endpoint {
    operationId: string;
    heyClientMethodName: string;
    title: string;
    method: string;
    path: string;
    paramDefs: ParamDefinition[];
    exampleParams: Record<string, Record<string, any>>;
    requestBody?: RequestBody;
    responseBodies: Record<string, ResponseBody | undefined>;
    streaming: boolean;
    tags: string[];
}

export type EndpointMap = Record<string, Endpoint>;

function getResponseBody(r: OpenAPIV3_1.ResponseObject, defs: SchemaMap): ResponseBody {
    const content = r.content as Record<string, any>;
    if (!content) throw new Error('Response body must have content defined');
    const keys = Object.keys(content);
    const jsonKey = keys.find(k => /json$/i.test(k));
    const mediaType = jsonKey || keys[0];
    const entry = content[mediaType];
    const schema = inlineSchemaWithExample(entry.schema, defs, entry.example) as InlinedSchema;
    const streaming = (/ndjson$/i.test(mediaType));
    return { mediaType, schema, streaming };
}

function getRequestBody(r: OpenAPIV3_1.RequestBodyObject, defs: SchemaMap): RequestBody {
    const content = r.content as Record<string, any>;
    const required = Boolean(r.required);
    if (!content) throw new Error('Request body must have content defined');
    const keys = Object.keys(content);
    const jsonKey = keys.find(k => /json$/i.test(k));
    const mediaType = jsonKey || keys[0];
    const entry = content[mediaType];
    const schema = inlineSchemaWithExample(entry.schema, defs, entry.example) as InlinedSchema;
    return { mediaType, required, schema };
}

function toCamel(s: string): string {
    return s.replace(/_([a-z])/g, (_, c) => c.toUpperCase());
}

function buildParamDef(param: OpenAPIV3_1.ParameterObject, defs: SchemaMap): ParamDefinition {
    const schema = inlineSchemaWithExample(param.schema!, defs, param.example) as InlinedSchema;
    return {
        name: param.name,
        in: param.in as any,
        required: Boolean(param.required),
        schema,
    };
}

function buildResponses(responses: OpenAPIV3_1.ResponsesObject | undefined, defs: SchemaMap) {
    const responseBodies: Record<string, ResponseBody | undefined> = {};
    for (const [code, resp] of Object.entries(responses || {})) {
        const r = resp as OpenAPIV3_1.ResponseObject;
        responseBodies[code] = r.content ? getResponseBody(r, defs) : undefined;
    }
    return responseBodies;
}
function buildExampleEndpointParams(
    paramDefs: ParamDefinition[],
    responseStreaming: boolean,
    requestBody?: RequestBody
): Record<string, Record<string, any>> {
    const required = paramDefs.filter(p => p.required);
    const optional = paramDefs.filter(p => !p.required);
    const variants: { title: string; params: ParamDefinition[] }[] = [
        { title: 'all', params: [...required] },
        ...optional.map(p => ({ title: p.name, params: [...required, p] }))
    ];

    const argsMap: Record<string, Record<string, any>> = {};

    variants.forEach(variant => {
        const { title, params } = variant;
        // build base args
        const args: any = { throwOnError: false };
        if (responseStreaming) args.parseAs = 'stream';
        // populate params
        params.forEach(p => {
            args[p.in] = args[p.in] || {};
            args[p.in][p.name] = p.schema.examples?.[0];
        });
        // handle body examples
        if (requestBody?.schema.examples) {
            requestBody.schema.examples.forEach((bodyEx, idx) => {
                const key = `${title}, body${idx}`;
                argsMap[key] = { ...args, body: bodyEx };
            });
        } else {
            argsMap[title] = args;
        }
    });

    return argsMap;
}

function buildEndpoint(path: string, method: string, op: OpenAPIV3_1.OperationObject, defs: SchemaMap): Endpoint {
    const operationId = op.operationId!;
    const heyClientMethodName = toCamel(operationId);
    const title = op.summary || op.description || operationId;
    const tags = op.tags || [];
    const parameters = op.parameters as OpenAPIV3_1.ParameterObject[] || [];

    const paramDefs = parameters.map(p => buildParamDef(p, defs));
    const requestBody =
        op.requestBody ? getRequestBody(op.requestBody as OpenAPIV3_1.RequestBodyObject, defs) : undefined;

    const responseBodies = buildResponses(op.responses, defs);
    const streaming = responseBodies['200']?.streaming || false;
    const exampleParams = buildExampleEndpointParams(paramDefs, streaming, requestBody);

    return {
        operationId,
        heyClientMethodName,
        title,
        method,
        path,
        paramDefs,
        exampleParams,
        requestBody,
        responseBodies,
        streaming,
        tags,
    };
}

export function generateEndpoints(raw: OpenAPIV3_1.PathsObject, defs: SchemaMap): EndpointMap {
    const map: EndpointMap = {};
    for (const [path, pathItem] of Object.entries(raw)) {
        if (!pathItem) continue;
        Object.entries(pathItem).forEach(([method, opObj]) => {
            const ep = buildEndpoint(path, method.toUpperCase(), opObj as OpenAPIV3_1.OperationObject, defs);
            map[ep.operationId] = ep;
        });
    }
    return map;
}
