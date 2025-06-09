use proc_macro2::{Ident, TokenStream};
use quote::quote;

pub fn delete_statement(table: &Ident) -> TokenStream {
    quote! {
        let mut table_col_3 = write_tx.open_table(#table)?;
        let _ = table_col_3.remove(pk)?;
    }
}

pub fn delete_many_statement(table: &Ident) -> TokenStream {
    quote! {
        let mut table_col_4 = write_tx.open_table(#table)?;
        for pk in pks.iter() {
            table_col_4.remove(pk)?;
        }
    }
}

pub fn delete_index_statement(table: &Ident, index_table: &Ident) -> TokenStream {
    quote! {
        let mut table_col_8 = write_tx.open_table(#table)?;
        let maybe_value = {
            if let Some(value_guard) = table_col_8.remove(pk)? {
                Some(value_guard.value().clone())
            } else {
                None
            }
        };
        if let Some(value) = maybe_value {
            let mut mm = write_tx.open_multimap_table(#index_table)?;
            mm.remove(&value, pk)?;
        }
    }
}

pub fn delete_many_index_statement(table: &Ident, index_table: &Ident) -> TokenStream {
    quote! {
        let mut table_col_9 = write_tx.open_table(#table)?;
        let mut mm = write_tx.open_multimap_table(#index_table)?;
        for pk in pks.iter() {
            if let Some(value_guard) = table_col_9.remove(pk)? {
                let value = value_guard.value();
                mm.remove(&value, pk)?;
            }
        }
    }
}

pub fn delete_dict_statement(table_dict_pk_by_pk: &Ident, table_value_to_dict_pk: &Ident, table_value_by_dict_pk: &Ident, table_dict_index: &Ident) -> TokenStream {
    quote! {
        let mut dict_pk_by_pk       = write_tx.open_table(#table_dict_pk_by_pk)?;
        let mut value_to_dict_pk    = write_tx.open_table(#table_value_to_dict_pk)?;
        let mut value_by_dict_pk    = write_tx.open_table(#table_value_by_dict_pk)?;
        let mut dict_index          = write_tx.open_multimap_table(#table_dict_index)?;

        let birth_id_opt = dict_pk_by_pk.remove(pk)?.map(|guard| guard.value().clone());
        if let Some(birth_id) = birth_id_opt {
            let value_opt = value_by_dict_pk.get(&birth_id)?.map(|guard| guard.value().clone());
            if let Some(value) = value_opt {
                dict_index.remove(&birth_id, pk)?;
                if dict_index.get(&birth_id)?.is_empty() {
                    value_to_dict_pk.remove(&value)?;
                    value_by_dict_pk.remove(&birth_id)?;
                }
            }
        }
    }
}

pub fn delete_many_dict_statement(table_dict_pk_by_pk: &Ident, table_value_to_dict_pk: &Ident, table_value_by_dict_pk: &Ident, table_dict_index: &Ident) -> TokenStream {
    quote! {
        let mut dict_pk_by_pk       = write_tx.open_table(#table_dict_pk_by_pk)?;
        let mut value_to_dict_pk    = write_tx.open_table(#table_value_to_dict_pk)?;
        let mut value_by_dict_pk    = write_tx.open_table(#table_value_by_dict_pk)?;
        let mut dict_index          = write_tx.open_multimap_table(#table_dict_index)?;

        for pk in pks.iter() {
            let birth_id_opt = dict_pk_by_pk.remove(pk)?.map(|guard| guard.value().clone());
            if let Some(birth_id) = birth_id_opt { // duplicate
                let value_opt = value_by_dict_pk.get(&birth_id)?.map(|guard| guard.value().clone());
                if let Some(value) = value_opt {
                    dict_index.remove(&birth_id, pk)?;
                    if dict_index.get(&birth_id)?.is_empty() {
                        value_to_dict_pk.remove(&value)?;
                        value_by_dict_pk.remove(&birth_id)?;
                    }
                }
            }
        }
    }
}
