#[cfg(feature = "chain")]

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
    let header_type = header_type_from(field_defs)?;

    Ok(quote! {
        impl BlockLike for #block_type {
            type Header = #header_type;
            fn header(&self) -> &Self::Header {
                &self.header
            }
        }

        #[async_trait::async_trait]
        impl chain::BlockChainLike<#block_type> for BlockChain {
            fn init(&self) -> Result<(), chain::ChainError> {
                Ok(Block::init(Arc::clone(&self.storage))?)
            }

            fn delete(&self) -> Result<(), chain::ChainError> {
                if let Some(tip_header) = #header_type::last(&self.storage.begin_read()?)? {
                    let write_tx = self.storage.begin_write()?;
                    for height in 1..=tip_header.height.0 {
                        #block_type::delete(&write_tx, &Height(height))?;
                    }
                    write_tx.commit()?;
                }
                Ok(())
            }

            fn get_last_header(&self) -> Result<Option<#header_type>, chain::ChainError> {
                let read_tx = self.storage.begin_read()?;
                let last = #header_type::last(&read_tx)?;
                Ok(last)
            }

            fn get_header_by_hash(&self, hash: [u8; 32]) -> Result<Vec<#header_type>, chain::ChainError> {
                let read_tx = self.storage.begin_read()?;
                let header = #header_type::get_by_hash(&read_tx, &BlockHash(hash))?;
                Ok(header)
            }

            fn store_blocks(&self, blocks: Vec<#block_type>) -> Result<(), chain::ChainError> {
                for block in &blocks {
                    #block_type::store_and_commit(Arc::clone(&self.storage), block)?;
                }
                Ok(())
            }

            async fn validate_chain(&self) -> Result<Vec<#header_type>, chain::ChainError> {
                use futures::StreamExt;
                let read_tx = self.storage.begin_read()?; // kept as-is even if unused
                let mut affected_headers: Vec<#header_type> = Vec::new();
                if let Some(tip_header) = #header_type::last(&read_tx)? {
                    let mut stream = #header_type::stream_range(self.storage.begin_read()?, Height(0), tip_header.height, None)?;

                    // get the first header (nothing to validate yet)
                    let mut prev = match stream.next().await {
                        Some(Ok(h)) => h,
                        Some(Err(e)) => return Err(chain::ChainError::new(format!("Stream error: {}", e))),
                        None => return Ok(Vec::new()), // empty chain
                    };

                    while let Some(item) = stream.next().await {
                        let curr = match item {
                            Ok(h) => h,
                            Err(e) => return Err(chain::ChainError::new(format!("Stream error: {}", e))),
                        };

                        if prev.hash != curr.prev_hash {
                           error!(
                             "Chain unlinked, curr {} @ {:?}, prev {} @ {:?}",
                             hex::encode(curr.prev_hash.0), curr.height, hex::encode(prev.hash.0), prev.height
                           );
                           affected_headers.push(prev.clone());
                        }
                        prev = curr;
                    }
                }
                Ok(affected_headers)
            }

            fn update_blocks(&self, blocks: Vec<#block_type>) -> Result<(), chain::ChainError> {
                let write_tx = self.storage.begin_write()?;
                for block in &blocks {
                    #block_type::delete(&write_tx, &block.height)?;
                }
                write_tx.commit()?;
                self.store_blocks(blocks)?;
                Ok(())
            }

            fn populate_inputs(&self, blocks: &mut Vec<Block>) -> Result<(), chain::ChainError> {
                let read_tx = self.storage.begin_read()?;
                for block in blocks.iter_mut() {
                    self.resolve_tx_inputs(&read_tx, block)?;
                }
                Ok(())
            }
        }
    })
}
