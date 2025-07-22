import { describe, it, expect, beforeAll } from 'vitest';
import type { OpenAPIV3_1 } from 'openapi-types';
import { generateEndpoints, Endpoint, ParamInfo } from './generateEndpoints';
import { SchemaMap } from './inlineSchema';
import { fetchSchema } from './schema';

// Mock definitions for schemas
const defs: SchemaMap = {
    StringObj: { type: 'object', properties: { foo: { type: 'string' } }, example: { foo: 'bar' } },
    NumArr: { type: 'array', items: { type: 'number' }, example: [1, 2, 3] },
};

let openapi: OpenAPIV3_1.Document;
let endpoints: ReturnType<typeof generateEndpoints>;

beforeAll(async () => {
    openapi = await fetchSchema('http://127.0.0.1:8000/apidoc/openapi.json');
    const defs: SchemaMap = openapi.components?.schemas as any;
    endpoints = generateEndpoints(openapi.paths!, defs);
});

describe('inlinePaths unit tests', () => {
    it('ignores paths without operationId', () => {
        const raw: OpenAPIV3_1.PathsObject = {
            '/nop': { get: {} as any }
        };
        const result = generateEndpoints(raw, defs);
        expect(Object.keys(result)).toHaveLength(0);
    });

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
        const result = generateEndpoints(raw, defs);
        expect(result).toHaveProperty('item_get');
        const ep: Endpoint = result.item_get;
        // methodName should be camelCase
        expect(ep.methodName).toBe('itemGet');
        expect(ep.method).toBe('GET');
        expect(ep.path).toBe('/item/{id}');
        // params
        expect(ep.params).toHaveLength(1);
        const p: ParamInfo = ep.params[0];
        expect(p.name).toBe('id');
        expect(p.in).toBe('path');
        expect(p.required).toBe(true);
        expect(p.example).toBe('xyz');
        // requestBody undefined
        expect(ep.requestBody).toBeUndefined();
        // response schema inlined
        expect(ep.responseSchemas['200']).toEqual(defs.StringObj);
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
        const result = generateEndpoints(raw, defs);
        expect(result).toHaveProperty('nums_post');
        const ep = result.nums_post;
        expect(ep.methodName).toBe('numsPost');
        // requestBody inlined
        expect(ep.requestBody).toBeDefined();
        expect(ep.requestBody!.schema).toEqual(defs.NumArr);
        expect(ep.requestBody!.example).toEqual([7,8,9]);
        // responses
        expect(ep.responseSchemas['201']?.type).toBe('boolean');
        expect(ep.responseSchemas['400']?.type).toBe('string');
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
        const result = generateEndpoints(raw, defs);
        const ep = result.simple_delete;
        expect(ep.params).toHaveLength(0);
        expect(ep.requestBody).toBeUndefined();
        // 204 with no content => no responseSchemas entry
        expect(ep.responseSchemas['204']).toBeUndefined();
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
        expect(ep?.responseSchemas['200']).toBeDefined();
        // either 404 or 500 should exist
        expect(
            ep?.responseSchemas['500'] || ep?.responseSchemas['404']
        ).toBeDefined();
    });

    it('includes POST /asset with query params', () => {
        const ep = Object.values(endpoints).find(
            (e) => e.path === '/asset' && e.method === 'GET'
        );
        // asset_limit operation
        expect(ep).toBeDefined();
        // should have multiple query params
        expect(ep?.params.some(p => p.in === 'query')).toBe(true);
    });
});
