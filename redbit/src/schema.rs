
use utoipa::openapi::{OpenApi, RefOr, Schema};
use serde_json::Map;
use std::collections::BTreeMap;

fn resolve_ref<'a>(
    ref_str: &str,
    defs: &'a BTreeMap<String, RefOr<Schema>>,
) -> Option<&'a RefOr<Schema>> {
    ref_str.strip_prefix("#/components/schemas/").and_then(|k| defs.get(k))
}

pub fn load(openapi: &OpenApi, root: String) -> serde_json::Result<String> {
    let defs = openapi.components.as_ref().unwrap().schemas.clone();
    println!("Inlining root schema : {}", root);
    let root_schema = defs.get(&root).expect("Root schema not found");
    let flat = inline_schema(root_schema, &defs);
    serde_json::to_string_pretty(&flat)
}

fn inline_schema(schema: &RefOr<Schema>, defs: &BTreeMap<String, RefOr<Schema>>) -> serde_json::Value {
    match schema {
        RefOr::Ref(reference) => {
            if let Some(resolved) = resolve_ref(&reference.ref_location, defs) {
                return inline_schema(resolved, defs);
            }
            panic!("Unresolved reference: {:?}", reference);
        }
        RefOr::T(schema_obj) => {
            match serde_json::to_value(schema_obj).expect("Failed to serialize schema") {
                serde_json::Value::Object(mut map) => {
                    // If "properties" exists, inline its values
                    if let Some(serde_json::Value::Object(props)) = map.remove("properties") {
                        let mut fields = Map::new();
                        for (k, v) in props {
                            fields.insert(k, inline_value_refs(v, defs));
                        }
                        map.insert("fields".to_string(), serde_json::Value::Object(fields));
                    }

                    if let Some(items) = map.remove("items") {
                        map.insert("items".to_string(), inline_value_refs(items, defs));
                    }
                    serde_json::Value::Object(map)
                }
                other => other,
            }
        }
    }
}

fn inline_value_refs(val: serde_json::Value, defs: &BTreeMap<String, RefOr<Schema>>) -> serde_json::Value {
    match val {
        serde_json::Value::Object(mut map) => {
            // Inline $ref first
            if let Some(serde_json::Value::String(reference)) = map.get("$ref") {
                if let Some(resolved) = resolve_ref(reference, defs) {
                    return inline_schema(resolved, defs);
                }
            }

            // Inline "properties" as "fields"
            if let Some(serde_json::Value::Object(props)) = map.get("properties") {
                let mut fields = Map::new();
                for (k, v) in props {
                    fields.insert(k.clone(), inline_value_refs(v.clone(), defs));
                }
                map.remove("properties");
                map.insert("fields".to_string(), serde_json::Value::Object(fields));
            }

            // Inline "items"
            if let Some(items_val) = map.remove("items") {
                map.insert("items".to_string(), inline_value_refs(items_val, defs));
            }

            // Inline "oneOf"
            if let Some(serde_json::Value::Array(one_of_arr)) = map.remove("oneOf") {
                let inlined = one_of_arr.into_iter()
                    .map(|v| inline_value_refs(v, defs))
                    .collect();
                map.insert("oneOf".to_string(), serde_json::Value::Array(inlined));
            }

            // Inline "anyOf"
            if let Some(serde_json::Value::Array(any_of_arr)) = map.remove("anyOf") {
                let inlined = any_of_arr.into_iter()
                    .map(|v| inline_value_refs(v, defs))
                    .collect();
                map.insert("anyOf".to_string(), serde_json::Value::Array(inlined));
            }

            // Inline "allOf"
            if let Some(serde_json::Value::Array(all_of_arr)) = map.remove("allOf") {
                let inlined = all_of_arr.into_iter()
                    .map(|v| inline_value_refs(v, defs))
                    .collect();
                map.insert("allOf".to_string(), serde_json::Value::Array(inlined));
            }

            serde_json::Value::Object(map)
        }
        other => other,
    }
}
