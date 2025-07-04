use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;

pub fn plain_init_expr(table: &Ident) -> TokenStream {
    quote! {
        {
            let table_col_5 = tx.open_table(#table)?;
            let guard = table_col_5.get(pk)?;
            guard.ok_or_else(|| AppError::NotFound(format!(
                    "table `{}`: no row for primary key {:?}",
                    stringify!(#table),
                    pk
                ))
            )?.value()
        }
    }
}

pub fn plain_init(column_name: &Ident, table: &Ident) -> TokenStream {
    let init_expr = plain_init_expr(table);
    quote! {
        #column_name: #init_expr
    }
}

pub fn plain_init_with_query(column_name: &Ident, table: &Ident) -> TokenStream {
    let init_expr = plain_init_expr(table);
    quote !{
        let #column_name = #init_expr;
        if let Some(expected) = streaming_query.#column_name.clone() {
            if expected != #column_name {
                return Ok(None);
            }
        }
    }
}

pub fn plain_default_init(column_name: &Ident, column_type: &Type) -> TokenStream {
    quote! {
        #column_name: #column_type::default()
    }
}


pub fn index_init_expr(table: &Ident) -> TokenStream {
    quote! {
        {
            let table_col_10 = tx.open_table(#table)?;
            let guard = table_col_10.get(pk)?;
            guard.ok_or_else(|| AppError::NotFound(format!(
                    "table `{}`: no row for primary key {:?}",
                    stringify!(#table),
                    pk
                ))
            )?.value()
        }
    }
}

pub fn index_init_with_query(column_name: &Ident, table: &Ident) -> TokenStream {
    let init_expr = index_init_expr(table);
    quote! {
        let #column_name = #init_expr;
        if let Some(expected) = streaming_query.#column_name.clone() {
            if expected != #column_name {
                return Ok(None);
            }
        }
    }
}

pub fn index_init(column_name: &Ident, table: &Ident) -> TokenStream {
    let init_expr = index_init_expr(table);
    quote! {
        #column_name: #init_expr
    }
}

pub fn index_default_init(column_name: &Ident, column_type: &Type) -> TokenStream {
    quote! {
        #column_name: {
            let mut value = <#column_type as Default>::default();
            for _ in 0..sample_index {
                value = <#column_type as IterableColumn>::next(&value);
            }
            value
        }
    }
}

pub fn dict_init_expr(table_dict_pk_by_pk: &Ident, table_value_by_dict_pk: &Ident) -> TokenStream {
    quote! {
        {
            let pk2birth = tx.open_table(#table_dict_pk_by_pk)?;
            let birth_guard = pk2birth.get(pk)?;
            let birth_id = birth_guard.ok_or_else(|| AppError::NotFound(format!(
                    "table_dict_pk_by_pk_ident `{}`: no row for primary key {:?}",
                    stringify!(#table_dict_pk_by_pk),
                    pk
                ))
            )?.value();
            let birth2val = tx.open_table(#table_value_by_dict_pk)?;
            let val_guard = birth2val.get(&birth_id)?;
            val_guard.ok_or_else(|| AppError::NotFound(format!(
                    "table_value_by_dict_pk `{}`: no row for birth id {:?}",
                    stringify!(#table_value_by_dict_pk),
                    birth_id
                ))
            )?.value()
        }
    }
}

pub fn dict_init(column_name: &Ident, table_dict_pk_by_pk: &Ident, table_value_by_dict_pk: &Ident) -> TokenStream {
    let init_expr = dict_init_expr(table_dict_pk_by_pk, table_value_by_dict_pk);
    quote! {
        #column_name: #init_expr
    }
}

pub fn dict_init_with_query(column_name: &Ident, table_dict_pk_by_pk: &Ident, table_value_by_dict_pk: &Ident) -> TokenStream {
    let init_expr = dict_init_expr(table_dict_pk_by_pk, table_value_by_dict_pk);
    quote! {
        let #column_name = #init_expr;
        if let Some(expected) = streaming_query.#column_name.clone() {
            if expected != #column_name {
                return Ok(None);
            }
        }
    }
}

pub fn dict_default_init(column_name: &Ident, column_type: &Type) -> TokenStream {
    quote! {
        #column_name: {
            let mut value = <#column_type as Default>::default();
            for _ in 0..sample_index {
                value = <#column_type as IterableColumn>::next(&value);
            }
            value
        }
    }
}
