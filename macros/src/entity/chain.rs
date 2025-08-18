use crate::field_parser::FieldDef;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Type;

#[derive(Clone, Copy, Debug)]
enum Expected {
    Unsigned(usize),   // u8/u16/u32/u64/u128 — unsigned only
    ArrayU8(usize),    // [u8; N]
}

/* ---------- validation helpers ---------- */

fn expected_hint(exp: Expected) -> String {
    match exp {
        Expected::Unsigned(b) => format!("a #[column] newtype wrapping `u{b}`"),
        Expected::ArrayU8(n)  => format!("a #[column] newtype wrapping `[u8; {n}]`"),
    }
}

fn find_field<'a>(fields: &'a [FieldDef], name: &str) -> Option<&'a FieldDef> {
    fields.iter().find(|f| f.name == name)
}

fn expect_field<'a>(
    span: Span,
    fields: &'a [FieldDef],
    name: &str,
    exp: Expected,
) -> Result<&'a Type, syn::Error> {
    fields
        .iter()
        .find(|f| f.name == name)
        .map(|f| &f.tpe)
        .ok_or_else(|| {
            syn::Error::new(
                span,
                format!(
                    "missing required field `{}` (help: add `pub {}: <YourNewtype>` where `<YourNewtype>: Column<Repr = {}>`)",
                    name, name, expected_hint(exp)
                ),
            )
        })
}

fn header_type_from(fields: &[FieldDef]) -> Result<&Type, syn::Error> {
    match find_field(fields, "header") {
        Some(fd) => Ok(&fd.tpe),
        None => Err(syn::Error::new(
            Span::call_site(),
            "missing required field `header` (help: add `pub header: <HeaderType>`)",
        )),
    }
}

fn impl_where_bounds(column_path: &TokenStream, wants: &[(&Type, Expected)]) -> Result<TokenStream, syn::Error> {
    let mut clauses = Vec::with_capacity(wants.len());
    for (ty, exp) in wants {
        let repr: Type = match *exp {
            Expected::Unsigned(bits) => syn::parse_str(&format!("u{bits}"))?,
            Expected::ArrayU8(n)     => syn::parse_str(&format!("[u8; {n}]"))?,
        };
        clauses.push(quote!( #ty: #column_path<Repr = #repr> ));
    }
    Ok(quote!( where #(#clauses,)* ))
}

pub fn block_header_like(header_type: Type, field_defs: &[FieldDef]) -> Result<TokenStream, syn::Error> {
    let span = Span::call_site();

    let height     = expect_field(span, field_defs, "height",     Expected::Unsigned(32))?;
    let hash       = expect_field(span, field_defs, "hash",       Expected::ArrayU8(32))?;
    let prev_hash  = expect_field(span, field_defs, "prev_hash",  Expected::ArrayU8(32))?;
    let timestamp  = expect_field(span, field_defs, "timestamp",  Expected::Unsigned(32))?;
    let weight     = expect_field(span, field_defs, "weight",     Expected::Unsigned(32))?;

    let col = quote!(redbit::ColInnerType);
    let where_bounds = impl_where_bounds(&col, &[
        (height,    Expected::Unsigned(32)),
        (hash,      Expected::ArrayU8(32)),
        (prev_hash, Expected::ArrayU8(32)),
        (timestamp, Expected::Unsigned(32)),
        (weight,    Expected::Unsigned(32)),
    ])?;

    Ok(quote! {
        impl BlockHeaderLike for #header_type #where_bounds {
            fn height(&self) -> u32         { self.height.0 }
            fn hash(&self) -> [u8; 32]      { self.hash.0 }
            fn prev_hash(&self) -> [u8; 32] { self.prev_hash.0 }
            fn timestamp(&self) -> u32      { self.timestamp.0 }
            fn weight(&self) -> u32         { self.weight.0 }
        }
    })
}

pub fn block_like(block_type: Type, field_defs: &[FieldDef]) -> Result<TokenStream, syn::Error> {
    let header_ty = header_type_from(field_defs)?;

    Ok(quote! {
        impl BlockLike for #block_type {
            type Header = #header_ty;
            fn header(&self) -> &Self::Header {
                &self.header
            }
        }
    })
}
