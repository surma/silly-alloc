use proc_macro2::{Group, Span, TokenStream, TokenTree};
use quote::{format_ident, quote, spanned::Spanned, ToTokens};
use syn::{
    braced,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token::Comma,
    Error, Expr, ExprBlock, ExprLit, ExprStruct, FieldValue, Ident, Index, Lit, LitInt, Member,
    Pat, Result, Token,
};

mod cast_helpers;
use cast_helpers::*;

struct BucketAllocatorDescriptor {
    name: Ident,
    buckets: Vec<BucketDescriptor>,
}

impl Parse for BucketAllocatorDescriptor {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name = Ident::parse(input)?;
        Comma::parse(input)?;
        let fields;
        braced!(fields in input);
        let items = Punctuated::<BucketDescriptor, Token![,]>::parse_terminated(&fields)?;
        Ok(BucketAllocatorDescriptor {
            name,
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

impl BucketDescriptor {
    fn as_init_values(&self) -> TokenStream {
        let BucketDescriptor {
            num_slots,
            slot_size,
            align,
        } = self;
        let slot_type_ident = Ident::new(
            &format!("SlotWithAlign{}", align.try_to_int_literal().unwrap()),
            align.__span(),
        );
        quote! {
            UnsafeCell::new(Bucket::new())
        }
        .into()
    }

    fn as_struct_fields(&self) -> TokenStream {
        let BucketDescriptor {
            num_slots,
            slot_size,
            align,
        } = self;
        let slot_type_ident = Ident::new(
            &format!("SlotWithAlign{}", align.try_to_int_literal().unwrap()),
            align.__span(),
        );
        quote! {
            UnsafeCell<Bucket<#slot_type_ident<#slot_size>, #num_slots>>
        }
        .into()
    }

    fn as_debug_print_stmts(&self, idx: usize) -> TokenStream {
        let BucketDescriptor {
            num_slots,
            slot_size,
            align,
        } = self;
        let size_str = slot_size.try_to_int_literal().unwrap();
        let idx_key = Index::from(idx);
        quote! {
            .field(#size_str, unsafe { &self.#idx_key.get().as_ref().unwrap() })
        }
        .into()
    }

    fn as_alloc_bucket_selectors(&self, idx: usize) -> TokenStream {
        let BucketDescriptor {
            num_slots,
            slot_size,
            align,
        } = self;
        let size = slot_size
            .try_to_int_literal()
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let idx_key = Index::from(idx);
        quote! {
            if size <= #size {
                if let Some(ptr) = self.#idx_key.get().as_mut().unwrap().take_first_available_slot() {
                    return ptr as *mut u8;
                }
            }
        }
        .into()
    }

    fn as_dealloc_bucket_selectors(&self, idx: usize) -> TokenStream {
        let BucketDescriptor {
            num_slots,
            slot_size,
            align,
        } = self;
        let size = slot_size
            .try_to_int_literal()
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let idx_key = Index::from(idx);
        quote! {
            if let Some(bucket) = self.#idx_key.get().as_mut() {
                if let Some(slot_idx) = bucket.slot_idx_for_ptr(ptr) {
                    bucket.unset_slot(slot_idx);
                }
            }
        }
        .into()
    }
}

#[proc_macro]
pub fn bucket_allocator(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let BucketAllocatorDescriptor { name, buckets } = parse_macro_input!(input);
    // TODO: Sort buckets?
    let name_str = stringify!(name);
    let bucket_field_decls: Vec<TokenStream> = buckets
        .iter()
        .map(|bucket| bucket.as_struct_fields())
        .collect();

    let bucket_field_inits: Vec<TokenStream> = buckets
        .iter()
        .map(|bucket| bucket.as_init_values())
        .collect();

    let bucket_field_dbg: Vec<TokenStream> = buckets
        .iter()
        .enumerate()
        .map(|(idx, bucket)| bucket.as_debug_print_stmts(idx))
        .collect();

    let alloc_bucket_selectors: Vec<TokenStream> = buckets
        .iter()
        .enumerate()
        .map(|(idx, bucket)| bucket.as_alloc_bucket_selectors(idx))
        .collect();

    let dealloc_bucket_selectors: Vec<TokenStream> = buckets
        .iter()
        .enumerate()
        .map(|(idx, bucket)| bucket.as_dealloc_bucket_selectors(idx))
        .collect();

    quote! {
            #[derive(Default)]
            struct #name(
                #(#bucket_field_decls),*
            );

            impl #name {
                const fn new() -> Self {
                    #name (
                        #(#bucket_field_inits),*
                    )
                }
            }

            impl Debug for #name {
                fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
                    f.debug_struct(#name_str)
                        #(#bucket_field_dbg)*
                        .finish()
                }
            }

            unsafe impl GlobalAlloc for #name {
                unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
                    // FIXME: Respect align
                    let size = layout.size();
                    #(#alloc_bucket_selectors)*
                    core::ptr::null_mut()
                }

                unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
                    // FIXME: Respect align
                    #(#dealloc_bucket_selectors)*
                }

            }
    }
    .into()
}
