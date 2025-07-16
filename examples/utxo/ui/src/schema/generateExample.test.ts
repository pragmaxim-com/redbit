import { generateExample } from "./generateExample";
import { describe, it, expect, beforeAll } from "vitest";
import {OpenAPIV3_1} from "openapi-types";

let openapi: OpenAPIV3_1.Document;

beforeAll(async () => {
    const res = await fetch("http://127.0.0.1:8000/apidoc/openapi.json");
    if (!res.ok) throw new Error("Failed to fetch OpenAPI schema");
    openapi = (await res.json()) as OpenAPIV3_1.Document;
});

describe("resolveSchema", () => {
    it("generates examples for a complex schema with refs", () => {
        const defs = openapi.components?.schemas;
        const root = openapi.components?.schemas?.Block;

        expect(defs).toBeDefined();
        expect(root).toBeDefined();

        const example = generateExample(root!, defs!);

        expect(example).toBeDefined();
        expect(typeof example).toBe("object");

        // Basic top-level fields
        expect(example).toHaveProperty("id");
        expect(example).toHaveProperty("transactions");

        // Nested array field
        expect(Array.isArray(example.transactions)).toBe(true);
        expect(example.transactions.length).toBeGreaterThan(0);
        expect(example.transactions[0]).toHaveProperty("hash");

        console.log(JSON.stringify(example, null, 2));
    });
});
