use proc_macro2::{Ident, TokenStream};
use quote::quote;

pub fn store_statement(pk_name: &Ident, column_name: &Ident, table: &Ident) -> TokenStream {
    quote! {
        let mut table_col_1 = tx.open_table(#table)?;
        table_col_1.insert(&instance.#pk_name, &instance.#column_name)?;
    }
}

pub fn store_many_statement(pk_name: &Ident, column_name: &Ident, table: &Ident) -> TokenStream {
    quote! {
        let mut table_col_2 = tx.open_table(#table)?;
        for instance in instances.iter() {
            table_col_2.insert(&instance.#pk_name, &instance.#column_name)?;
        }
    }
}

pub fn store_index_def(column_name: &Ident, pk_name: &Ident, table: &Ident, index_table: &Ident) -> TokenStream {
    quote! {
        let mut table_col_6 = tx.open_table(#table)?;
        table_col_6.insert(&instance.#pk_name, &instance.#column_name)?;

        let mut mm = tx.open_multimap_table(#index_table)?;
        mm.insert(&instance.#column_name, &instance.#pk_name)?;
    }
}

pub fn store_many_index_def(column_name: &Ident, pk_name: &Ident, table: &Ident, index_table: &Ident) -> TokenStream {
    quote! {
        let mut table_col_7 = tx.open_table(#table)?;
        let mut mm = tx.open_multimap_table(#index_table)?;
        for instance in instances.iter() {
            table_col_7.insert(&instance.#pk_name, &instance.#column_name)?;
            mm.insert(&instance.#column_name, &instance.#pk_name)?;
        };
    }
}

pub fn store_dict_def(column_name: &Ident, pk_name: &Ident, table_dict_pk_by_pk: &Ident, table_value_to_dict_pk: &Ident, table_value_by_dict_pk: &Ident, table_dict_index: &Ident) -> TokenStream {
    quote! {
        let mut dict_pk_by_pk       = tx.open_table(#table_dict_pk_by_pk)?;
        let mut value_to_dict_pk    = tx.open_table(#table_value_to_dict_pk)?;
        let mut value_by_dict_pk    = tx.open_table(#table_value_by_dict_pk)?;
        let mut dict_index          = tx.open_multimap_table(#table_dict_index)?;

        let (birth_id, newly_created) = {
            if let Some(guard) = value_to_dict_pk.get(&instance.#column_name)? {
                (guard.value().clone(), false)
            } else {
                let new_birth = instance.#pk_name.clone();
                (new_birth, true)
            }
        };

        if newly_created {
            value_to_dict_pk.insert(&instance.#column_name, &birth_id)?;
            value_by_dict_pk.insert(&birth_id, &instance.#column_name)?;
        }

        dict_pk_by_pk.insert(&instance.#pk_name, &birth_id)?;
        dict_index.insert(&birth_id, &instance.#pk_name)?;
    }
}

pub fn store_many_dict_def(column_name: &Ident, pk_name: &Ident, table_dict_pk_by_pk: &Ident, table_value_to_dict_pk: &Ident, table_value_by_dict_pk: &Ident, table_dict_index: &Ident) -> TokenStream {
    quote! {
        let mut dict_pk_by_pk       = tx.open_table(#table_dict_pk_by_pk)?;
        let mut value_to_dict_pk    = tx.open_table(#table_value_to_dict_pk)?;
        let mut value_by_dict_pk    = tx.open_table(#table_value_by_dict_pk)?;
        let mut dict_index          = tx.open_multimap_table(#table_dict_index)?;

        for instance in instances.iter() {
             let (birth_id, newly_created) = {
                if let Some(guard) = value_to_dict_pk.get(&instance.#column_name)? {
                    (guard.value().clone(), false)
                } else {
                    let new_birth = instance.#pk_name.clone();
                    (new_birth, true)
                }
            };

            if newly_created {
                value_to_dict_pk.insert(&instance.#column_name, &birth_id)?;
                value_by_dict_pk.insert(&birth_id, &instance.#column_name)?;
            }

            dict_pk_by_pk.insert(&instance.#pk_name, &birth_id)?;
            dict_index.insert(&birth_id, &instance.#pk_name)?;
        }
    }
}
