use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;

pub fn default_init_expr(column_type: &Type) -> TokenStream {
    quote! {
        {
            let value = <#column_type as Default>::default();
            <#column_type as IterableColumn>::nth_value(&value, sample_index)
        }
    }
}

pub fn default_init(column_name: &Ident, column_type: &Type) -> TokenStream {
    let default_expr = default_init_expr(column_type);
    quote! {
        let #column_name = #default_expr;
    }
}

pub fn default_init_with_query(column_name: &Ident, column_type: &Type) -> TokenStream {
    let default_expr = default_init_expr(column_type);
    quote! {
        let #column_name = #default_expr;
        if let Some(filter_op) = stream_query.#column_name.clone() && !filter_op.matches(&#column_name) {
            return None;
        }
    }
}

pub fn plain_init_expr(table: &Ident) -> TokenStream {
    quote! {
        {
            let guard = tx_context.#table.get_value(pk)?;
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
        let #column_name = #init_expr;
    }
}

pub fn plain_init_with_query(column_name: &Ident, table: &Ident) -> TokenStream {
    let init_expr = plain_init_expr(table);
    quote! {
        let #column_name = #init_expr;
        if let Some(filter_op) = stream_query.#column_name.clone() && !filter_op.matches(&#column_name) {
            return Ok(None);
        }
    }
}

pub fn index_init_expr(plain_table_var: &Ident) -> TokenStream {
    quote! {
        {
            let guard = tx_context.#plain_table_var.get_value(pk)?;
            guard.ok_or_else(|| AppError::NotFound(format!(
                    "table `{}`: no row for primary key {:?}",
                    stringify!(#plain_table_var),
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
        if let Some(filter_op) = stream_query.#column_name.clone() && !filter_op.matches(&#column_name) {
            return Ok(None);
        }
    }
}

pub fn index_init(column_name: &Ident, table: &Ident) -> TokenStream {
    let init_expr = index_init_expr(table);
    quote! {
        let #column_name = #init_expr;
    }
}

pub fn dict_init_expr(dict_table_var: &Ident) -> TokenStream {
    quote! {
        {
            let value_guard_opt = tx_context.#dict_table_var.get_value(pk)?;
            value_guard_opt.ok_or_else(|| AppError::NotFound(format!(
                    "dict_table `{}`: no row for primary key {:?}",
                    stringify!(#dict_table_var),
                    pk
                ))
            )?.value()
        }
    }
}

pub fn dict_init(column_name: &Ident, dict_table_var: &Ident) -> TokenStream {
    let init_expr = dict_init_expr(dict_table_var);
    quote! {
        let #column_name = #init_expr;
    }
}

pub fn dict_init_with_query(column_name: &Ident, dict_table_var: &Ident) -> TokenStream {
    let init_expr = dict_init_expr(dict_table_var);
    quote! {
        let #column_name = #init_expr;
        if let Some(filter_op) = stream_query.#column_name.clone() && !filter_op.matches(&#column_name) {
            return Ok(None);
        }
    }
}

