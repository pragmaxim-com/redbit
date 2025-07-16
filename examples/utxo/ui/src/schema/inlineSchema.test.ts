import {inlineSchema} from "./inlineSchema";
import { describe, it, expect, beforeAll } from "vitest";
import {OpenAPIV3_1} from "openapi-types";
import {fetchSchema} from "./schema";

let openapi: OpenAPIV3_1.Document;

beforeAll(async () => {
    openapi = await fetchSchema("http://127.0.0.1:8000/apidoc/openapi.json")
});

function isArraySchemaObject(
    schema: unknown
): schema is OpenAPIV3_1.ArraySchemaObject {
    return (
        typeof schema === "object" &&
        schema !== null &&
        "type" in schema &&
        (schema as any).type === "array" &&
        "items" in schema
    );
}

describe("resolveSchema", () => {
    it("resolves refs and transforms schema correctly", () => {
        const defs = openapi.components?.schemas;

        expect(defs).toBeDefined();

        const inlined = inlineSchema("Block", defs!);

        expect("properties" in inlined).toBe(true);
        const props = (inlined as OpenAPIV3_1.SchemaObject).properties!;
        expect(props).toHaveProperty("id");

        const transactions = props["transactions"];
        expect(transactions).toBeDefined();
        expect(isArraySchemaObject(transactions)).toBe(true);

        if (!isArraySchemaObject(transactions)) {
            throw new Error("Expected transactions to be an array schema");
        }

        const items = transactions.items!;
        expect("properties" in items).toBe(true);

        const itemProps = (items as OpenAPIV3_1.SchemaObject).properties!;
        expect(itemProps).toHaveProperty("hash");

        const refsLeft = JSON.stringify(inlined).match(/\$ref/);
        expect(refsLeft).toBeNull();
    });

});
