import { describe, it, expect } from 'vitest';
import type { OpenAPIV3_1 } from 'openapi-types';
import {generateEndpoints, Endpoint, ParamDefinition} from './generateEndpoints';
import {fetchSchema, SchemaMap} from './schema';
import * as client from "../hey";

const mockDefs: SchemaMap = {
    StringObj: { type: 'object', properties: { foo: { type: 'string' } }, example: { foo: 'bar' } },
    NumArr: { type: 'array', items: { type: 'number' }, examples: [[7,8,9]] },
};

const openapi: OpenAPIV3_1.Document = await fetchSchema('http://127.0.0.1:8000/apidoc/openapi.json');
const realDefs: SchemaMap = openapi.components?.schemas as any;
const endpoints = generateEndpoints(openapi.paths!, realDefs);
const realEndpoints: Endpoint[] = Object.values(endpoints).filter(ep => ep.method !== 'DELETE');


describe('Hey-API JSON client calls', () => {
    it('has endpoints to test', () => {
        expect(realEndpoints.length).toBeGreaterThan(0);
    });

    realEndpoints.forEach(ep => {
        ep.exampleParams.forEach(param => {
            it(`${ep.heyClientMethodName}() → ${ep.method} ${ep.path}`, async () => {
                const { data, response, error } = await (client as any)[ep.heyClientMethodName](param);
                if (response.status !== 200) {
                    console.error(`Error calling ${ep.streaming} ${ep.heyClientMethodName}(${JSON.stringify(param)})`);
                    console.error('Response:', response);
                    console.error('Error:', error);
                }

                expect(response.status).toBe(200);
                expect(error).toBeUndefined();
                expect(data).toBeDefined();
            });

            if (ep.streaming) {
                it(`${ep.heyClientMethodName}() → ${ep.method} ${ep.path}`, async () => {
                    param.body = null;
                    const { data, response, error } = await (client as any)[ep.heyClientMethodName](param);
                    if (response.status !== 200) {
                        console.error(`Error calling ${ep.streaming} ${ep.heyClientMethodName}(${JSON.stringify(param)})`);
                        console.error('Response:', response);
                        console.error('Error:', error);
                    }
                    expect(response.status).toBe(200);
                    expect(error).toBeUndefined();
                    expect(data).toBeDefined();
                 });
            }
        });
    });
});


describe('inlinePaths unit tests', () => {
    it('parses a GET with path param and response', () => {
        const raw: OpenAPIV3_1.PathsObject = {
            '/item/{id}': {
                get: {
                    operationId: 'item_get',
                    summary: 'Get item',
                    parameters: [
                        { name: 'id', in: 'path', required: true, schema: { type: 'string' }, example: 'xyz' }
                    ],
                    responses: {
                        '200': { content: { 'application/json': { schema: { $ref: '#/components/schemas/StringObj' } } } }
                    }
                } as any
            }
        };
        const result = generateEndpoints(raw, mockDefs);
        expect(result).toHaveProperty('item_get');
        const ep: Endpoint = result.item_get;
        // methodName should be camelCase
        expect(ep.heyClientMethodName).toBe('itemGet');
        expect(ep.method).toBe('GET');
        expect(ep.path).toBe('/item/{id}');
        // params
        expect(ep.paramDefs).toHaveLength(1);
        const p: ParamDefinition = ep.paramDefs[0];
        expect(p.name).toBe('id');
        expect(p.in).toBe('path');
        expect(p.required).toBe(true);
        expect(p.schema.examples![0]).toBe('xyz');
        // requestBody undefined
        expect(ep.requestBody).toBeUndefined();
        // response schema inlined
        expect(ep.responseBodies['200']).toBeDefined();
    });

    it('parses POST with requestBody and multiple responses', () => {
        const raw: OpenAPIV3_1.PathsObject = {
            '/nums': {
                post: {
                    operationId: 'nums_post',
                    summary: 'Post numbers',
                    requestBody: {
                        content: {
                            'application/json': { schema: { $ref: '#/components/schemas/NumArr' }, example: [7,8,9] }
                        }
                    },
                    responses: {
                        '201': { content: { 'application/json': { schema: { type: 'boolean' } } } },
                        '400': { description: 'Bad', content: { 'application/json': { schema: { type: 'string' } } } }
                    }
                } as any
            }
        };
        const result = generateEndpoints(raw, mockDefs);
        expect(result).toHaveProperty('nums_post');
        const ep = result.nums_post;
        expect(ep.heyClientMethodName).toBe('numsPost');
        // requestBody inlined
        expect(ep.requestBody).toBeDefined();
        expect(ep.requestBody!.schema).toEqual(mockDefs.NumArr);
        expect(ep.requestBody!.schema.examples![0]).toEqual([7,8,9]);
        // responses
        expect(ep.responseBodies['201']?.mediaType).toBe('application/json');
        expect(ep.responseBodies['400']?.mediaType).toBe('application/json');
    });

    it('handles endpoints with no parameters or body', () => {
        const raw: OpenAPIV3_1.PathsObject = {
            '/simple': {
                delete: {
                    operationId: 'simple_delete',
                    summary: 'Delete simple',
                    responses: { '204': { description: 'No Content' } }
                } as any
            }
        };
        const result = generateEndpoints(raw, mockDefs);
        const ep = result.simple_delete;
        expect(ep.paramDefs).toHaveLength(0);
        expect(ep.requestBody).toBeUndefined();
        // 204 with no content => no responseSchemas entry
        expect(ep.responseBodies['204']).toBeUndefined();
    });
});

describe('inlinePaths with real OpenAPI schema', () => {
    it('parses at least one endpoint', () => {
        expect(Object.keys(endpoints).length).toBeGreaterThan(0);
    });

    it('includes GET /block/id/{id} with correct response schemas', () => {
        const ep = Object.values(endpoints).find(
            (e) => e.path === '/block/id/{id}' && e.method === 'GET'
        );
        expect(ep).toBeDefined();
        expect(ep?.responseBodies['200']).toBeDefined();
        // either 404 or 500 should exist
        expect(
            ep?.responseBodies['500'] || ep?.responseBodies['404']
        ).toBeDefined();
    });

    it('includes POST /asset with query params', () => {
        const ep = Object.values(endpoints).find(
            (e) => e.path === '/asset' && e.method === 'GET'
        );
        // asset_limit operation
        expect(ep).toBeDefined();
        // should have multiple query params
        expect(ep?.paramDefs.some(p => p.in === 'query')).toBe(true);
    });
});
