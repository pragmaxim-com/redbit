import { generateExample } from "./generateExample";
import { describe, it, expect, beforeAll } from "vitest";
import {OpenAPIV3_1} from "openapi-types";
import {fetchSchema} from "./schema";

let openapi: OpenAPIV3_1.Document;

beforeAll(async () => {
    openapi = await fetchSchema("http://127.0.0.1:8000/apidoc/openapi.json")
});

describe("generateExample", () => {
    it("generates examples for a complex schema with refs", () => {
        const defs = openapi.components?.schemas;

        expect(defs).toBeDefined();

        const example = generateExample("Block", defs!);

        expect(example).toBeDefined();
        expect(typeof example).toBe("object");

        // Basic top-level fields
        expect(example).toHaveProperty("id");
        expect(example).toHaveProperty("transactions");

        // Nested array field
        expect(Array.isArray(example.transactions)).toBe(true);
        expect(example.transactions.length).toBeGreaterThan(0);
        expect(example.transactions[0]).toHaveProperty("hash");

        // console.log(JSON.stringify(example, null, 2));
    });
});
