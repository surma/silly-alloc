use proc_macro2::{Group, Span, TokenStream, TokenTree};
use quote::{quote, spanned::Spanned, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    Error, Expr, ExprLit, ExprStruct, FieldValue, Lit, LitInt, Member, Pat, Result, Token,
};

struct BucketAllocatorDescriptor {
    buckets: Vec<BucketDescriptor>,
}

impl Parse for BucketAllocatorDescriptor {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let parse = Punctuated::<BucketDescriptor, Token![,]>::parse_terminated;
        let items = parse(input)?;
        Ok(BucketAllocatorDescriptor {
            buckets: items.into_iter().collect(),
        })
    }
}

struct BucketDescriptor {
    slot_size: Expr,
    align: Expr,
    num_slots: Expr,
}

fn path_references_item(p: &syn::Path, item: &str) -> bool {
    p.segments
        .iter()
        .last()
        .unwrap()
        .to_token_stream()
        .to_string()
        .as_str()
        == item
}

fn find_field<'a>(s: &'a ExprStruct, name: &str) -> Option<&'a FieldValue> {
    s.fields.iter().find(|field| {
        let Member::Named(field_name) = &field.member else {return false;};
        field_name.to_string().as_str() == name
    })
}

impl Parse for BucketDescriptor {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let bucket_desc = ExprStruct::parse(input)?;
        assert!(
            path_references_item(&bucket_desc.path, "Bucket"),
            "Items in a BucketAllocator must be buckets."
        );

        let slot_size = find_field(&bucket_desc, "slot_size")
            .ok_or_else(|| Error::new(bucket_desc.__span(), "slot_size is mandatory"))?
            .expr
            .clone();
        let num_slots = find_field(&bucket_desc, "num_slots")
            .ok_or_else(|| Error::new(bucket_desc.__span(), "num_slots is mandatory"))?
            .expr
            .clone();
        let align = find_field(&bucket_desc, "align")
            .map(|field| &field.expr)
            .cloned()
            .unwrap_or_else(|| {
                Expr::Lit(ExprLit {
                    attrs: vec![],
                    lit: Lit::Int(LitInt::new("1usize", bucket_desc.__span())),
                })
            });

        Ok(BucketDescriptor {
            slot_size,
            align,
            num_slots,
        })
    }
}

#[proc_macro]
pub fn bucket_allocator(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut bucket: BucketAllocatorDescriptor = parse_macro_input!(input);
    // TODO: Sort buckets?

    let bucket_fields: Vec<TokenStream> = bucket
        .buckets
        .iter()
        .enumerate()
        .map(|(idx, bucket)| {
            let BucketDescriptor {
                num_slots,
                slot_size,
                align,
            } = bucket;
            quote! {
                UnsafeCell<Bucket<SlotWithAlign2<#slot_size>, #num_slots>>
            }
        })
        .collect();

    quote! {
        #[derive(Default)]
        struct BucketAllocator(
            #(#bucket_fields),*
        );
    }
    .into()
}
