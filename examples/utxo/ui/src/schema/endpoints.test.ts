import * as client from '../hey/';
import { generateExample } from './generateExample';
import { describe, it, expect } from 'vitest';
import type { OpenAPIV3_1 } from 'openapi-types';
import { Endpoint, generateEndpoints } from './generateEndpoints';
import { SchemaMap } from './inlineSchema';
import { fetchSchema } from './schema';

// prepare
const openapi: OpenAPIV3_1.Document = await fetchSchema('http://127.0.0.1:8000/apidoc/openapi.json');
const defs: SchemaMap = openapi.components?.schemas as any;
const endpointsMap = generateEndpoints(openapi.paths!, defs);

// filter JSON, non-streaming and sort DELETE methods last
const testEndpoints: Endpoint[] = Object.values(endpointsMap)
    .filter(ep => !ep.streaming && ep.method !== 'DELETE');

function generateParamArgs(params: Endpoint['params'], defs: SchemaMap) {
    const args: any = {};
    for (const p of params) {
        const loc = p.in;
        args[loc] = args[loc] || {};
        const key = p.refName || p.name;
        args[loc][p.name] = p.example !== undefined ? p.example : generateExample(key, defs);
    }
    return args;
}

function generateBodyArg(ep: Endpoint, defs: SchemaMap) {
    if (!ep.requestBody) return undefined;
    const key = ep.requestBody.refName || ep.operationId;
    return ep.requestBody.example !== undefined ? ep.requestBody.example : generateExample(key, defs);
}

function buildParamVariants(ep: Endpoint) {
    const requiredParams = ep.params.filter(p => p.required);
    const optionalParams = ep.params.filter(p => !p.required);

    return [
        {
            title: 'all params',
            params: [...requiredParams, ...optionalParams],
        },
        ...optionalParams.map(p => ({
            title: `param: ${p.name}`,
            params: [...requiredParams, p],
        }))
    ];
}

describe('Hey-API JSON client calls', () => {
    it('has endpoints to test', () => {
        expect(testEndpoints.length).toBeGreaterThan(0);
    });

    testEndpoints.forEach(ep => {
        const variants = buildParamVariants(ep);

        variants.forEach(variant => {
            it(`${ep.methodName}() → ${ep.method} ${ep.path} (${variant.title})`, async () => {
                const args: any = { throwOnError: false, ...generateParamArgs(variant.params, defs) };
                const body = generateBodyArg(ep, defs);
                if (body !== undefined) args.body = body;

                const fn = (client as any)[ep.methodName];
                expect(typeof fn).toBe('function');

                const { data, response, error } = await fn(args);
                if (response.status !== 200) {
                    console.error(`Error calling ${ep.methodName}(${JSON.stringify(args)})`);
                    console.error('Response:', response);
                    console.error('Error:', error);
                }

                expect(response.status).toBe(200);
                expect(error).toBeUndefined();
                expect(data).toBeDefined();
            });
        });
    });
});
