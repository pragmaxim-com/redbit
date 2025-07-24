import type { OpenAPIV3_1 } from "openapi-types";
import {inlineSchemaWithExample, SchemaMap} from "./schema";

export type InlinedSchema = OpenAPIV3_1.SchemaObject;

export interface ParamDefinition {
    name: string;
    in: "path" | "query" | "header" | "cookie";
    required: boolean;
    schema: InlinedSchema;
}

interface Body {
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
    exampleParams: Record<string, any>[];
    requestBody?: Body;
    responseBodies: Record<string, Body | undefined>;
    streaming: boolean;
    tags: string[];
}

export type EndpointMap = Record<string, Endpoint>;

function getBody(content: Record<string, any>, defs: SchemaMap): Body {
    if (!content) throw new Error('Body must have content defined');
    const keys = Object.keys(content);
    const jsonKey = keys.find(k => /json$/i.test(k));
    const mediaType = jsonKey || keys[0];
    const entry = content[mediaType];
    const schema = inlineSchemaWithExample(entry.schema, defs, entry.example) as InlinedSchema;
    const streaming = (/ndjson$/i.test(mediaType));
    return { mediaType, schema, streaming };
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
    const responseBodies: Record<string, Body | undefined> = {};
    for (const [code, resp] of Object.entries(responses || {})) {
        const r = resp as OpenAPIV3_1.ResponseObject;
        responseBodies[code] = r.content ? getBody(r.content, defs) : undefined;
    }
    return responseBodies;
}

function buildExampleEndpointParams(
    paramDefs: ParamDefinition[],
    streaming: boolean,
    requestBody?: Body
): Record<string, any>[] {
    const required = paramDefs.filter(p => p.required);
    const optional = paramDefs.filter(p => !p.required);
    const variants: ParamDefinition[][] = [
        [...required],
        ...optional.map(p => [...required, p])
    ];
    return variants.map(paramsList => {
        const args: any = streaming ? { throwOnError: false, parseAs: 'stream' } : { throwOnError: false };
        paramsList.forEach(p => {
            args[p.in] = args[p.in] || {};
            args[p.in][p.name] = p.schema.examples![0];
        });
        if (requestBody) {
            args.body = requestBody.schema.examples![0];
        }
        return args;
    });
}

function buildEndpoint(path: string, method: string, op: OpenAPIV3_1.OperationObject, defs: SchemaMap): Endpoint {
    const operationId = op.operationId!;
    const heyClientMethodName = toCamel(operationId);
    const title = op.summary || op.description || operationId;
    const tags = op.tags || [];
    const parameters = op.parameters as OpenAPIV3_1.ParameterObject[] || [];

    const paramDefs = parameters.map(p => buildParamDef(p, defs));
    const requestBody =
        op.requestBody ? getBody((op.requestBody as OpenAPIV3_1.RequestBodyObject).content, defs) : undefined;

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
