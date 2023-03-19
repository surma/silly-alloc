use proc_macro2::TokenStream;
use quote::{quote, spanned::Spanned};
use syn::{
    parse::{Parse, ParseStream},
    *,
};

mod cast_helpers;
use cast_helpers::*;

struct BucketAllocatorDescriptor {
    name: Ident,
    buckets: Vec<BucketDescriptor>,
}

impl Parse for BucketAllocatorDescriptor {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let st: ItemStruct = input.parse()?;
        let name = st.ident;
        let buckets: Vec<Result<BucketDescriptor>> =
            st.fields.iter().map(|field| field.try_into()).collect();
        let buckets: Vec<BucketDescriptor> = Result::from_iter(buckets)?;
        Ok(BucketAllocatorDescriptor { name, buckets })
    }
}

struct BucketDescriptor {
    name: Ident,
    slot_size: Expr,
    align: Expr,
    num_slots: Expr,
}

impl TryFrom<&Field> for BucketDescriptor {
    type Error = syn::Error;
    fn try_from(field: &Field) -> Result<Self> {
        let name = field
            .ident
            .as_ref()
            .ok_or(Error::new(field.__span(), "Struct field without a name."))?;

        let Type::Path(path_type) = &field.ty else {return Err(Error::new(field.__span(), "Struct field’s type must have the simple type name 'Bucket'."))};
        if path_type.path.segments.len() != 1 {
            return Err(Error::new(
                path_type.__span(),
                "Struct field’s type must have the simple type name 'Bucket'.",
            ));
        }
        let path_seg = path_type.path.segments.iter().nth(0).unwrap();
        if path_seg.ident.to_string() != "Bucket" {
            return Err(Error::new(
                path_seg.__span(),
                "Struct field’s type must have the simple type name 'Bucket'.",
            ));
        }

        let mut slot_size: Option<Expr> = None;
        let mut num_slots: Option<Expr> = None;
        let mut align: Option<Expr> = None;
        let PathArguments::AngleBracketed(generics) = &path_seg.arguments else { return Err(Error::new(path_seg.__span(), "Bucket is missing generic arguments"))};
        for generic_arg in &generics.args {
            let GenericArgument::Type(Type::Path(param_type))= generic_arg else {return Err(Error::new( generic_arg.__span(), "Bucket can only take type arguments."))};
            if param_type.path.segments.len() != 1 {
                return Err(Error::new(
                    param_type.__span(),
                    "Invalid value for a Bucket property",
                ));
            }
            let segment = param_type.path.segments.iter().nth(0).unwrap();
            let param_name = &segment.ident;
            let PathArguments::AngleBracketed(param_generic_args) = &segment.arguments else {
            return Err(Error::new(segment.__span(), "Bucket parameters are passed as generic arguments."))
            };
            if param_generic_args.args.len() != 1 {
                return Err(Error::new(
                    param_generic_args.__span(),
                    "Bucket parameters take exactly one generic argument.",
                ));
            }
            let param_generic_arg = param_generic_args.args.iter().nth(0).unwrap();
            let GenericArgument::Const(expr) = param_generic_arg else {
            return Err(Error::new(param_generic_arg.__span(), "Bucket parameters must be a const expr."))
            };
            match param_name.to_string().as_str() {
                "SlotSize" => slot_size = Some(expr.clone()),
                "NumSlots" => num_slots = Some(expr.clone()),
                "Align" => align = Some(expr.clone()),
                _ => {
                    return Err(Error::new(
                        name.__span(),
                        format!("Unknown bucket parameter: {}", param_name.to_string()),
                    ))
                }
            };
        }

        Ok(BucketDescriptor {
            name: name.clone(),
            slot_size: slot_size
                .ok_or(Error::new(generics.__span(), "SlotSlize was not specified"))?,
            num_slots: num_slots
                .ok_or(Error::new(generics.__span(), "NumSlots was not specified"))?,
            align: align.unwrap_or_else(|| {
                Expr::Lit(ExprLit {
                    attrs: vec![],
                    lit: Lit::Int(LitInt::new("1usize", generics.__span())),
                })
            }),
        })
    }
}

impl BucketDescriptor {
    fn num_segments(&self) -> usize {
        ((self
            .num_slots
            .try_to_int_literal()
            .unwrap()
            .parse::<usize>()
            .unwrap() as f32)
            / 32.0)
            .ceil() as usize
    }

    fn as_init_values(&self) -> TokenStream {
        quote! {
            ::core::cell::UnsafeCell::new(BucketImpl::new())
        }
        .into()
    }

    fn as_struct_fields(&self) -> TokenStream {
        let BucketDescriptor {
            slot_size, align, ..
        } = self;
        let num_segments = self.num_segments();
        let slot_type_ident = Ident::new(
            &format!("SlotWithAlign{}", align.try_to_int_literal().unwrap()),
            align.__span(),
        );
        quote! {
            ::core::cell::UnsafeCell<BucketImpl<#slot_type_ident<#slot_size>, #num_segments>>
        }
        .into()
    }

    fn as_debug_print_stmts(&self, idx: usize) -> TokenStream {
        let BucketDescriptor { name, .. } = self;

        let idx_key = Index::from(idx);
        let name = name.to_string();
        quote! {
            .field(#name, unsafe { &self.#idx_key.get().as_ref().unwrap() })
        }
        .into()
    }

    fn as_alloc_bucket_selectors(&self, idx: usize) -> TokenStream {
        let BucketDescriptor { slot_size, .. } = self;
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

#[proc_macro_attribute]
pub fn bucket_allocator(
    _attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let BucketAllocatorDescriptor { name, buckets } = parse_macro_input!(input);
    // TODO: Sort buckets?
    let name_str = name.to_string();
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

            unsafe impl ::core::alloc::GlobalAlloc for #name {
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
