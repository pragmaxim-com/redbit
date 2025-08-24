use proc_macro2::{Ident, TokenStream};
use quote::quote;
use crate::column::open_dict_tables;
use crate::table::{DictTableDefs, StoreManyStmnt};

pub fn store_statement(pk_name: &Ident, column_name: &Ident, table: &Ident) -> TokenStream {
    let table_var = Ident::new(&format!("{}_table", table).to_lowercase(), table.span());
    quote! {
        let mut #table_var = tx.open_table(#table)?;
        #table_var.insert(&instance.#pk_name, &instance.#column_name)?;
    }
}

pub fn store_many_statement(pk_name: &Ident, column_name: &Ident, table: &Ident) -> StoreManyStmnt {
    let table_var = Ident::new(&format!("{}_table", table).to_lowercase(), table.span());
    StoreManyStmnt {
        pre: quote! {
            let mut #table_var = tx.open_table(#table)?;
        },
        insert: quote! {
            #table_var.insert(&instance.#pk_name, &instance.#column_name)?;
        },
        post: quote!{}
    }
}

pub fn store_index_def(column_name: &Ident, pk_name: &Ident, table: &Ident, index_table: &Ident) -> TokenStream {
    let table_var = Ident::new(&format!("{}_table", table).to_lowercase(), table.span());
    let mm_var = Ident::new(&format!("{}_index_table", index_table).to_lowercase(), index_table.span());
    quote! {
        let mut #table_var = tx.open_table(#table)?;
        #table_var.insert(&instance.#pk_name, &instance.#column_name)?;

        let mut #mm_var = tx.open_multimap_table(#index_table)?;
        #mm_var.insert(&instance.#column_name, &instance.#pk_name)?;
    }
}
pub fn store_many_index_def(column_name: &Ident, pk_name: &Ident, table: &Ident, index_table: &Ident) -> StoreManyStmnt {
    let table_var = Ident::new(&format!("{}_table", table).to_lowercase(), table.span());
    let mm_var = Ident::new(&format!("{}_index_table", index_table).to_lowercase(), index_table.span());

    StoreManyStmnt {
        pre: quote! {
            let mut #table_var = tx.open_table(#table)?;
            let mut #mm_var = tx.open_multimap_table(#index_table)?;
        },
        insert: quote! {
            #table_var.insert(&instance.#pk_name, &instance.#column_name)?;
            #mm_var.insert(&instance.#column_name, &instance.#pk_name)?;
        },
        post: quote!{}
    }
}

fn store_dict_stmnt(column_name: &Ident, pk_name: &Ident, cache: Option<Ident>, dict_pk_by_pk_var: Ident, value_to_dict_pk_var: Ident, value_by_dict_pk_var: Ident, dict_index_var: Ident) -> TokenStream {
    match cache {
        Some(cache_name) => quote! {
            let (birth_id, newly_created) =
                tx.cache_get_or_put(&#cache_name, &instance.#column_name, || {
                    if let Some(guard) = #value_to_dict_pk_var.get(&instance.#column_name)? {
                        Ok((guard.value(), false))
                    } else {
                        Ok((instance.#pk_name, true))
                    }
                })?;

            if newly_created {
                #value_to_dict_pk_var.insert(&instance.#column_name, &birth_id)?;
                #value_by_dict_pk_var.insert(&birth_id, &instance.#column_name)?;
            }

            #dict_pk_by_pk_var.insert(&instance.#pk_name, &birth_id)?;
            #dict_index_var.insert(&birth_id, &instance.#pk_name)?;
        },
        None => quote! {
            let (birth_id, newly_created) = {
                if let Some(guard) = #value_to_dict_pk_var.get(&instance.#column_name)? {
                    (guard.value(), false)
                } else {
                    let new_birth = instance.#pk_name;
                    (new_birth, true)
                }
            };

            if newly_created {
                #value_to_dict_pk_var.insert(&instance.#column_name, &birth_id)?;
                #value_by_dict_pk_var.insert(&birth_id, &instance.#column_name)?;
            }

            #dict_pk_by_pk_var.insert(&instance.#pk_name, &birth_id)?;
            #dict_index_var.insert(&birth_id, &instance.#pk_name)?;
        }
    }
}

pub fn store_dict_def(column_name: &Ident, pk_name: &Ident, dict_table_defs: &DictTableDefs) -> TokenStream {
    let (opened_tables, dict_pk_by_pk_var, value_to_dict_pk_var, value_by_dict_pk_var, dict_index_var) =
        open_dict_tables(dict_table_defs);
    let store_dict = store_dict_stmnt(column_name, pk_name, dict_table_defs.value_to_dict_pk_cache.clone(), dict_pk_by_pk_var, value_to_dict_pk_var, value_by_dict_pk_var, dict_index_var);
    quote! {
        #opened_tables
        #store_dict
    }
}

pub fn store_many_dict_def(column_name: &Ident, pk_name: &Ident, dict_table_defs: &DictTableDefs) -> StoreManyStmnt {
    let (opened_tables, dict_pk_by_pk_var, value_to_dict_pk_var, value_by_dict_pk_var, dict_index_var) =
        open_dict_tables(dict_table_defs);
    let store_dict = store_dict_stmnt(column_name, pk_name, dict_table_defs.value_to_dict_pk_cache.clone(), dict_pk_by_pk_var, value_to_dict_pk_var, value_by_dict_pk_var, dict_index_var);
    StoreManyStmnt {
        pre: quote! {
            #opened_tables
        },
        insert: quote! {
            #store_dict
        },
        post: quote!{}
    }
}
