#![cfg(feature = "chain")]

use crate::field_parser::FieldDef;
use proc_macro2::{Ident, Span, TokenStream};
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

fn field_type_from(fields: &[FieldDef], field_name: &str) -> Result<Type, syn::Error> {
    match find_field(fields, field_name) {
        Some(fd) => Ok(fd.tpe.clone()),
        None => Err(syn::Error::new(
            Span::call_site(),
            "missing required field `hash` (help: add `pub hash: <HeaderType>`)",
        )),
    }
}



pub fn block_header_like(header_type: Type, field_defs: &[FieldDef]) -> Result<TokenStream, syn::Error> {
    let span = Span::call_site();

    let hash_type  = field_type_from(field_defs, "hash")?;
    let timestamp_type  = field_type_from(field_defs, "timestamp")?;

    let height_inner_ty = expect_field(span, field_defs, "height", Expected::Unsigned(32))?;
    let hash_inner_ty = expect_field(span, field_defs, "hash", Expected::ArrayU8(32))?;
    let prev_hash_inner_ty = expect_field(span, field_defs, "prev_hash", Expected::ArrayU8(32))?;
    let timestamp_inner_ty = expect_field(span, field_defs, "timestamp", Expected::Unsigned(32))?;
    let weight_inner_ty = expect_field(span, field_defs, "weight", Expected::Unsigned(32))?;

    let col = quote!(redbit::ColInnerType);
    let where_bounds = impl_where_bounds(&col, &[
        (height_inner_ty, Expected::Unsigned(32)),
        (hash_inner_ty, Expected::ArrayU8(32)),
        (prev_hash_inner_ty, Expected::ArrayU8(32)),
        (timestamp_inner_ty, Expected::Unsigned(32)),
        (weight_inner_ty, Expected::Unsigned(32)),
    ])?;

    Ok(quote! {
        impl BlockHeaderLike for #header_type #where_bounds {
            type Hash = #hash_type;
            type TS = #timestamp_type;
            fn height(&self) -> u32                 { self.height.0 }
            fn hash(&self) -> #hash_type            { self.hash }
            fn prev_hash(&self) -> #hash_type       { self.prev_hash }
            fn timestamp(&self) -> #timestamp_type  { self.timestamp }
            fn weight(&self) -> u32                 { self.weight.0 }
        }
    })
}

pub fn block_like(block_type: Type, pk_name: &Ident, pk_type: &Type, field_defs: &[FieldDef], write_tx_context: &Type) -> Result<TokenStream, syn::Error> {
    let header_type = field_type_from(field_defs, "header")?;

    Ok(quote! {
        impl BlockLike for #block_type {
            type Header = #header_type;
            fn header(&self) -> &Self::Header {
                &self.header
            }
        }

        pub struct BlockChain {
            pub storage: Arc<Storage>,
        }

        impl BlockChain {
            pub fn new(storage: Arc<Storage>) -> Arc<dyn BlockChainLike<#block_type, #write_tx_context>> {
                Arc::new(BlockChain { storage })
            }
        }

        #[async_trait::async_trait]
        impl chain::BlockChainLike<#block_type, #write_tx_context> for BlockChain {
            fn init(&self) -> Result<(), chain::ChainError> {
                Ok(#block_type::init(Arc::clone(&self.storage))?)
            }

            fn new_indexing_ctx(&self) -> Result<#write_tx_context, chain::ChainError> {
                #block_type::new_write_ctx(&self.storage).map_err(|e| chain::ChainError::Custom(format!("Failed to create new indexing context: {}", e)))
            }

            fn delete(&self) -> Result<(), chain::ChainError> {
                let tx_context = #header_type::begin_read_ctx(&self.storage)?;
                if let Some(tip_header) = #header_type::last(&tx_context)? {
                    let tx_context = #block_type::begin_write_ctx(&self.storage, Durability::Immediate)?;
                    let pks = #pk_type::from_many(&(0..=tip_header.#pk_name.0).collect::<Vec<u32>>());
                    #block_type::delete_many(&tx_context, &pks)?;
                    tx_context.two_phase_commit_and_close()?;
                }
                Ok(())
            }

            fn get_last_header(&self) -> Result<Option<#header_type>, chain::ChainError> {
                let tx_context = #header_type::begin_read_ctx(&self.storage)?;
                let last = #header_type::last(&tx_context)?;
                Ok(last)
            }

            fn get_header_by_hash(&self, hash: <<Block as BlockLike>::Header as BlockHeaderLike>::Hash) -> Result<Vec<#header_type>, chain::ChainError> {
                let tx_context = #header_type::begin_read_ctx(&self.storage)?;
                let header = #header_type::get_by_hash(&tx_context, &hash)?;
                Ok(header)
            }

            fn store_blocks(&self, indexing_context: &#write_tx_context, blocks: Vec<#block_type>, durability: Durability) -> Result<HashMap<String, TaskResult>, chain::ChainError> {
                let _ = indexing_context.begin_writing(durability)?;
                let master_start = Instant::now();
                #block_type::store_many(&indexing_context, blocks, true)?;
                let master_took = master_start.elapsed().as_millis();
                let mut tasks = indexing_context.two_phase_commit()?;

                let master_task = TaskResult::master(master_took);
                tasks.insert(master_task.name.clone(), master_task);
                Ok(tasks)
            }

            fn update_blocks(&self, indexing_context: &#write_tx_context, blocks: Vec<#block_type>) -> Result<HashMap<String, TaskResult>, chain::ChainError> {
                let _ = indexing_context.begin_writing(Durability::Immediate)?;
                for block in &blocks {
                    #block_type::delete(&indexing_context, block.#pk_name)?;
                }
                let _ = indexing_context.two_phase_commit()?;
                let result = self.store_blocks(indexing_context, blocks, Durability::Immediate)?;
                Ok(result)
            }

            async fn validate_chain(&self, validation_from_height: u32) -> Result<Vec<#header_type>, chain::ChainError> {
                use futures::StreamExt;
                let tx_context = #header_type::begin_read_ctx(&self.storage)?; // kept as-is even if unused
                let mut affected_headers: Vec<#header_type> = Vec::new();
                if let Some(tip_header) = #header_type::last(&tx_context)? {
                    let mut stream = #header_type::stream_range(tx_context, #pk_type(validation_from_height), tip_header.#pk_name, None)?;

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
                             hex::encode(curr.prev_hash.0), curr.#pk_name, hex::encode(prev.hash.0), prev.#pk_name
                           );
                           affected_headers.push(prev.clone());
                        }
                        prev = curr;
                    }
                }
                Ok(affected_headers)
            }
        }
    })
}
