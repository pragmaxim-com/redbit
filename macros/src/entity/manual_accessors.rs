use crate::field_parser::{ColumnDef, ColumnProps, EntityDef, FieldDef, IndexingType, KeyDef, Multiplicity, WriteFrom};
use std::collections::HashMap;
use crate::macro_utils;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

fn column_props_tokens(props: &ColumnProps) -> TokenStream {
    let shards = props.shards;
    let db_cache_weight = props.db_cache_weight;
    let lru_cache_size = props.lru_cache_size;
    quote! { redbit::schema::ColumnProps { shards: #shards, db_cache_weight: #db_cache_weight, lru_cache_size: #lru_cache_size } }
}

fn field_def_tokens(field: &FieldDef) -> TokenStream {
    let name = field.name.to_string();
    let ty = &field.tpe;
    quote! { redbit::schema::FieldDef { name: #name, ty: redbit::schema::TypeInfo::of::<#ty>() } }
}

fn multiplicity_tokens(mult: &Multiplicity) -> TokenStream {
    match mult {
        Multiplicity::OneToOne => quote!(redbit::schema::Multiplicity::OneToOne),
        Multiplicity::OneToOption => quote!(redbit::schema::Multiplicity::OneToOption),
        Multiplicity::OneToMany => quote!(redbit::schema::Multiplicity::OneToMany),
    }
}

fn schema_column_tokens(col: &ColumnDef) -> TokenStream {
    match col {
        ColumnDef::Key(KeyDef::Pk { field_def, column_props }) => {
            let fd = field_def_tokens(field_def);
            let props = column_props_tokens(column_props);
            quote! { redbit::schema::ColumnDef::Key(redbit::schema::KeyDef::Pk { field_def: #fd, column_props: #props }) }
        }
        ColumnDef::Key(KeyDef::Fk { field_def, multiplicity, parent_type, column_props }) => {
            let fd = field_def_tokens(field_def);
            let props = column_props_tokens(column_props);
            let mult = multiplicity_tokens(multiplicity);
            let parent = parent_type
                .as_ref()
                .map(|p| quote! { Some(redbit::schema::TypeInfo::of::<#p>()) })
                .unwrap_or_else(|| quote!(None));
            quote! { redbit::schema::ColumnDef::Key(redbit::schema::KeyDef::Fk { field_def: #fd, multiplicity: #mult, parent_type: #parent, column_props: #props }) }
        }
        ColumnDef::Plain(field_def, idx, used, pointer) => {
            let fd = field_def_tokens(field_def);
            let idx_tokens = match idx {
                IndexingType::Off(props) => {
                    let p = column_props_tokens(props);
                    quote!(redbit::schema::IndexingType::Off(#p))
                }
                IndexingType::Index(props) => {
                    let p = column_props_tokens(props);
                    quote!(redbit::schema::IndexingType::Index(#p))
                }
                IndexingType::Range(props) => {
                    let p = column_props_tokens(props);
                    quote!(redbit::schema::IndexingType::Range(#p))
                }
                IndexingType::Dict(props) => {
                    let p = column_props_tokens(props);
                    quote!(redbit::schema::IndexingType::Dict(#p))
                }
            };
            let used_tokens = if used.is_some() { quote!(Some(redbit::schema::Used)) } else { quote!(None) };
            quote! { redbit::schema::ColumnDef::Plain(#fd, #idx_tokens, #used_tokens, #pointer) }
        }
        ColumnDef::Relationship(field_def, write_from, used, mult) => {
            let fd = field_def_tokens(field_def);
            let mult_tokens = multiplicity_tokens(mult);
            let wf_tokens = write_from
                .as_ref()
                .map(|wf| {
                    let from = wf.from.to_string();
                    let using = wf.using.to_string();
                    quote! { Some(redbit::schema::WriteFrom { from: #from, using: #using }) }
                })
                .unwrap_or_else(|| quote!(None));
            let used_tokens = if used.is_some() { quote!(Some(redbit::schema::Used)) } else { quote!(None) };
            quote! { redbit::schema::ColumnDef::Relationship(#fd, #wf_tokens, #used_tokens, #mult_tokens) }
        }
        ColumnDef::Transient(field_def) => {
            let fd = field_def_tokens(field_def);
            quote! { redbit::schema::ColumnDef::Transient(#fd) }
        }
        ColumnDef::TransientRel(field_def, read_from) => {
            let fd = field_def_tokens(field_def);
            let rf_tokens = read_from
                .as_ref()
                .map(|rf| {
                    let outer = rf.outer.to_string();
                    let inner = rf.inner.to_string();
                    quote! { Some(redbit::schema::ReadFrom { outer: #outer, inner: #inner }) }
                })
                .unwrap_or_else(|| quote!(None));
            quote! { redbit::schema::ColumnDef::TransientRel(#fd, #rf_tokens) }
        }
    }
}

pub fn emit(entity_def: &EntityDef, cols: &[ColumnDef]) -> TokenStream {
    let entity_name = &entity_def.entity_name;
    let pk_field = entity_def.key_def.field_def();
    let pk_type = &pk_field.tpe;
    let pk_ident = &pk_field.name;
    let cascade_helper_ident = format_ident!("__{}CascadeHelper", entity_name);
    let pk_type_is_pointer = match pk_type {
        syn::Type::Path(p) => p.path.segments.last().map(|s| s.ident.to_string().ends_with("Pointer")).unwrap_or(false),
        _ => false,
    };
    let pk_parent_type_tokens: Option<TokenStream> = match &entity_def.key_def {
        KeyDef::Fk { .. } if pk_type_is_pointer => Some(quote! { <#pk_type as redbit::ChildPointer>::Parent }),
        KeyDef::Fk { .. } => Some(quote! { #pk_type }),
        _ => None,
    };
    let pk_is_child = pk_parent_type_tokens.is_some();

    let schema_columns: Vec<TokenStream> = cols.iter().map(schema_column_tokens).collect();

    let mut deps_fields: Vec<TokenStream> = Vec::new();
    let mut accessor_pushes: Vec<TokenStream> = Vec::new();
    let mut auto_child_inits: Vec<TokenStream> = Vec::new();
    let mut auto_dep_fields: Vec<TokenStream> = Vec::new();
    let mut auto_writer_children: Vec<TokenStream> = Vec::new();
    let mut type_lookup: HashMap<String, syn::Type> = HashMap::new();
    let mut auto_errors: Vec<TokenStream> = Vec::new();

    for col in cols {
        match col {
            ColumnDef::Plain(field_def, idx, used, _) => {
                let field_ident = &field_def.name;
                let field_name = field_ident.to_string();
                type_lookup.insert(field_name.clone(), field_def.tpe.clone());
                match idx {
                    IndexingType::Off(_) => accessor_pushes.push(quote! {
                        accessors.push(redbit::manual_entity::plain_accessor(
                            #field_name,
                            |e: &#entity_name| e.#field_ident.clone(),
                            |mut e: #entity_name, v| { e.#field_ident = v; e }
                        ));
                    }),
                    IndexingType::Index(_) | IndexingType::Range(_) => {
                        let accessor_fn = if used.is_some() {
                            quote!(redbit::manual_entity::index_used_accessor)
                        } else {
                            quote!(redbit::manual_entity::index_accessor)
                        };
                        accessor_pushes.push(quote! {
                            accessors.push(#accessor_fn(
                                #field_name,
                                |e: &#entity_name| e.#field_ident.clone(),
                                |mut e: #entity_name, v| { e.#field_ident = v; e }
                            ));
                        });
                    }
                    IndexingType::Dict(_) => accessor_pushes.push(quote! {
                        accessors.push(redbit::manual_entity::dict_accessor(
                            #field_name,
                            |e: &#entity_name| e.#field_ident.clone(),
                            |mut e: #entity_name, v| { e.#field_ident = v; e }
                        ));
                    }),
                }
            }
            ColumnDef::Relationship(field_def, write_from, _used, mult) => {
                let field_ident = &field_def.name;
                let field_name = field_ident.to_string();
                let child_type = &field_def.tpe;
                let child_pk_alias = match child_type {
                    syn::Type::Path(p) => {
                        p.path.segments.last().map(|s| format_ident!("{}Pk", s.ident)).unwrap_or_else(|| format_ident!("__UnknownPk"))
                    }
                    _ => format_ident!("__UnknownPk"),
                };
                let child_alias_str = child_pk_alias.to_string();
                let child_rt_var = format_ident!("{}_rt_auto", field_ident);
                if child_alias_str.starts_with("__UnknownPk") {
                    auto_errors.push(quote! { compile_error!("manual_runtime_auto could not infer child PK alias; ensure child type is a path"); });
                }
            if let Some(WriteFrom { from, using }) = write_from {
                    let hook_fn_manual = format_ident!("write_from_{}_using_{}_manual", from, using);
                    let from_field = from.clone();
                    let using_name = using.to_string();
                    let using_ty = match type_lookup.get(&using_name) {
                        Some(t) => quote! { #t },
                        None => {
                            auto_errors.push(quote! { compile_error!("write_from using field type not found"); });
                            quote! { () }
                        }
                    };
                    accessor_pushes.push(quote! {
                        accessors.push(redbit::manual_entity::write_from_accessor(
                            #field_name,
                            redbit::schema::TypeInfo::of::<#child_type>(),
                            std::sync::Arc::new(move |parent_writers: &redbit::manual_entity::RuntimeWriters<#entity_name, #pk_type>, child_writers: Option<&dyn redbit::manual_entity::ChildWriters>, parents: &[(#pk_type, #entity_name)]| {
                                let hash_batch = parent_writers.column_batches.iter().filter_map(|b| b.as_ref()).find_map(|b| b.as_any().downcast_ref::<redbit::manual_entity::IndexBatch<#entity_name, #pk_type, #using_ty>>()).ok_or_else(|| redbit::AppError::Custom("write_from: missing hash batch".into()))?;
                                // Collect input_refs -> (id pointer, idx)
                                let mut entries = redbit::indexmap::IndexMap::with_capacity(parents.iter().map(|(_, p)| p.#from_field.len()).sum());
                                for (pk, parent) in parents {
                                    for (idx, val) in parent.#from_field.iter().cloned().enumerate() {
                                        use redbit::indexmap::map::Entry;
                                        match entries.entry(val) {
                                            Entry::Vacant(v) => { v.insert((*pk, idx)); }
                                            Entry::Occupied(_) => return Err(redbit::AppError::Custom("Double spend not supported".into())),
                                        }
                                    }
                                }
                                let input_refs: Vec<_> = entries.iter().map(|(k, v)| (k.clone(), *v)).collect();
                                crate::hook::#hook_fn_manual(hash_batch, child_writers, input_refs, true)
                            })
                        ));
                    });
                    auto_writer_children.push(quote! {
                        let child_writers = #child_type::manual_writers_auto(storage)?;
                        writer_children.push((#field_name, Box::new(child_writers) as Box<dyn redbit::manual_entity::ChildWriters>));
                    });
                } else {
                    auto_child_inits.push(quote! { let #child_rt_var = #child_type::manual_runtime_auto()?; });
                    let dep_pk_ident = child_pk_alias.clone();
                    let runtime_field = format_ident!("{}_runtime", field_ident);
                    match mult {
                        Multiplicity::OneToOne => {
                            let map_field = format_ident!("{}_pk", field_ident);
                            deps_fields.push(quote! { pub #runtime_field: std::sync::Arc<redbit::manual_entity::ManualEntityRuntime<#child_type, #dep_pk_ident>> });
                            deps_fields.push(quote! { pub #map_field: fn(#pk_type) -> #dep_pk_ident });
                            accessor_pushes.push(quote! {
                                accessors.push(redbit::manual_entity::one2one_cascade_accessor(
                                    #field_name,
                                    std::sync::Arc::clone(&deps.#runtime_field),
                                    deps.#map_field,
                                    |p: &#entity_name| Some(p.#field_ident.clone()),
                                    |mut p: #entity_name, v| { if let Some(child) = v { p.#field_ident = child; } p }
                                ));
                            });
                            let map_expr = quote! { #cascade_helper_ident::map_identity::<#pk_type> };
                            auto_dep_fields.push(quote! { #runtime_field: std::sync::Arc::clone(&#child_rt_var) });
                            auto_dep_fields.push(quote! { #map_field: #map_expr });
                            auto_writer_children.push(quote! {
                                let child_writers = #child_type::manual_writers_auto(storage)?;
                                writer_children.push((#field_name, Box::new(child_writers) as Box<dyn redbit::manual_entity::ChildWriters>));
                            });
                        }
                        Multiplicity::OneToOption => {
                            let map_field = format_ident!("{}_pk", field_ident);
                            deps_fields.push(quote! { pub #runtime_field: std::sync::Arc<redbit::manual_entity::ManualEntityRuntime<#child_type, #dep_pk_ident>> });
                            deps_fields.push(quote! { pub #map_field: fn(#pk_type) -> #dep_pk_ident });
                            accessor_pushes.push(quote! {
                                accessors.push(redbit::manual_entity::one2one_cascade_accessor(
                                    #field_name,
                                    std::sync::Arc::clone(&deps.#runtime_field),
                                    deps.#map_field,
                                    |p: &#entity_name| p.#field_ident.clone(),
                                    |mut p: #entity_name, v| { p.#field_ident = v; p }
                                ));
                            });
                            let map_expr = quote! { #cascade_helper_ident::map_identity::<#pk_type> };
                            auto_dep_fields.push(quote! { #runtime_field: std::sync::Arc::clone(&#child_rt_var) });
                            auto_dep_fields.push(quote! { #map_field: #map_expr });
                            auto_writer_children.push(quote! {
                                let child_writers = #child_type::manual_writers_auto(storage)?;
                                writer_children.push((#field_name, Box::new(child_writers) as Box<dyn redbit::manual_entity::ChildWriters>));
                            });
                        }
                        Multiplicity::OneToMany => {
                            let map_field = format_ident!("{}_pk_at", field_ident);
                            deps_fields.push(quote! { pub #runtime_field: std::sync::Arc<redbit::manual_entity::ManualEntityRuntime<#child_type, #dep_pk_ident>> });
                            deps_fields.push(quote! { pub #map_field: fn(#pk_type, usize) -> #dep_pk_ident });
                            accessor_pushes.push(quote! {
                                accessors.push(redbit::manual_entity::one2many_cascade_accessor(
                                    #field_name,
                                    std::sync::Arc::clone(&deps.#runtime_field),
                                    std::sync::Arc::new(deps.#map_field),
                                    |p: &#entity_name| p.#field_ident.clone(),
                                    |mut p: #entity_name, v| { p.#field_ident = v; p }
                                ));
                            });
                            let map_expr = quote! { #cascade_helper_ident::map_child::<#pk_type, #dep_pk_ident> };
                            auto_dep_fields.push(quote! { #runtime_field: std::sync::Arc::clone(&#child_rt_var) });
                            auto_dep_fields.push(quote! { #map_field: #map_expr });
                            auto_writer_children.push(quote! {
                                let child_writers = #child_type::manual_writers_auto(storage)?;
                                writer_children.push((#field_name, Box::new(child_writers) as Box<dyn redbit::manual_entity::ChildWriters>));
                            });
                        }
                    }
                }
            }
            ColumnDef::Transient(_) | ColumnDef::TransientRel(_, _) | ColumnDef::Key(_) => {}
        }
    }

    let deps_ident = format_ident!("{}ManualDeps", macro_utils::to_camel_case(&entity_name.to_string(), true));

    let deps_struct = if deps_fields.is_empty() {
        quote! { pub struct #deps_ident; }
    } else {
        quote! { pub struct #deps_ident { #( #deps_fields, )* } }
    };

    let deps_param = if deps_fields.is_empty() {
        quote! { _deps: #deps_ident }
    } else {
        quote! { deps: #deps_ident }
    };

    let fn_generics = quote! {};

    let pk_alias = format_ident!("{}Pk", entity_name);
    let pk_parent_fn = quote! {};
    let pk_props_tokens = match &entity_def.key_def {
        KeyDef::Pk { column_props, .. } => column_props_tokens(column_props),
        KeyDef::Fk { column_props, .. } => column_props_tokens(column_props),
    };
    let pk_name_str = pk_ident.to_string();
    let entity_str = entity_name.to_string();
    let auto_deps_val = if auto_dep_fields.is_empty() {
        quote! { #deps_ident }
    } else {
        quote! { #deps_ident { #(#auto_dep_fields,)* } }
    };
    let auto_child_block = if auto_dep_fields.is_empty() {
        quote! {}
    } else {
        quote! { #(#auto_child_inits)* let deps_auto = #auto_deps_val; }
    };
    let accessors_auto = if auto_dep_fields.is_empty() {
        quote! { #entity_name::manual_accessors(#deps_ident) }
    } else {
        quote! { #entity_name::manual_accessors(deps_auto) }
    };

    quote! {
        pub type #pk_alias = #pk_type;
        #deps_struct
        impl #entity_name {
            pub const MANUAL_PK_IS_CHILD: bool = #pk_is_child;
            #pk_parent_fn
            pub fn manual_schema() -> Vec<redbit::schema::ColumnDef> {
                vec![ #(#schema_columns),* ]
            }

            pub fn manual_accessors #fn_generics(#deps_param) -> Vec<redbit::manual_entity::Accessor<#entity_name, #pk_type>>
            {
                let mut accessors = Vec::new();
                #(#accessor_pushes)*
                accessors
            }

            pub fn manual_runtime_auto() -> Result<std::sync::Arc<redbit::manual_entity::ManualEntityRuntime<#entity_name, #pk_type>>, redbit::AppError> {
                #(#auto_errors)*
                struct #cascade_helper_ident;
                impl #cascade_helper_ident {
                    fn map_identity<PK: Copy>(pk: PK) -> PK { pk }
                    fn map_child<PK: Copy, CK>(pk: PK, idx: usize) -> CK
                    where
                        CK: redbit::ChildPointer<Parent=PK>,
                        <CK as redbit::IndexedPointer>::Index: TryFrom<usize>,
                    {
                        let index: <CK as redbit::IndexedPointer>::Index = idx.try_into().unwrap_or_default();
                        <CK as redbit::ChildPointer>::from_parent(pk, index)
                    }
                }
                #auto_child_block
                let accessors = #accessors_auto;
                let pk_binding = redbit::manual_entity::PlainTableBinding::pk(#entity_str, #pk_name_str, #pk_props_tokens, true);
                let runtime = redbit::manual_entity::build_runtime_from_accessors(
                    #entity_str,
                    pk_binding,
                    #pk_name_str,
                    |e: &#entity_name| e.#pk_ident,
                    |pk: &#pk_type| #entity_name { #pk_ident: *pk, ..Default::default() },
                    accessors,
                    #entity_name::manual_schema(),
                )?;
                Ok(runtime)
            }

            pub fn manual_writers_auto(storage: &std::sync::Arc<redbit::storage::init::Storage>) -> Result<redbit::manual_entity::RuntimeWritersWithChildren<#entity_name, #pk_type>, redbit::AppError> {
                let rt = #entity_name::manual_runtime_auto()?;
                let self_writers = rt.begin_writers(storage)?;
                let mut writer_children: Vec<( &'static str, Box<dyn redbit::manual_entity::ChildWriters> )> = Vec::new();
                #(#auto_writer_children)*
                Ok(redbit::manual_entity::RuntimeWritersWithChildren { self_writers, children: writer_children })
            }
        }
    }
}
