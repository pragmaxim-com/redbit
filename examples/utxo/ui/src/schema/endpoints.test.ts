import * as client from '../hey/';
import { describe, it, expect } from 'vitest';
import type { OpenAPIV3_1 } from 'openapi-types';
import {Endpoint, generateEndpoints} from './generateEndpoints';
import { SchemaMap } from './inlineSchema';
import { fetchSchema } from './schema';

const openapi: OpenAPIV3_1.Document = await fetchSchema('http://127.0.0.1:8000/apidoc/openapi.json');
const defs: SchemaMap = openapi.components?.schemas as any;
const endpointsMap = generateEndpoints(openapi.paths!, defs);

const testEndpoints: Endpoint[] = Object.values(endpointsMap).filter(ep => ep.method !== 'DELETE');

describe('Hey-API JSON client calls', () => {
    it('has endpoints to test', () => {
        expect(testEndpoints.length).toBeGreaterThan(0);
    });

    testEndpoints.forEach(ep => {
        ep.exampleParams.forEach(param => {
            it(`${ep.methodName}() → ${ep.method} ${ep.path}`, async () => {
                const { data, response, error } = await (client as any)[ep.methodName](param);
                if (response.status !== 200) {
                    console.error(`Error calling ${ep.streaming} ${ep.methodName}(${JSON.stringify(param)})`);
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
