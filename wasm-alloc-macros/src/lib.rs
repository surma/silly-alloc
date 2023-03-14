use proc_macro2::{Group, TokenStream, TokenTree};
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    Error, ExprStruct, Member, Pat, Result, Token,
};

struct BucketAllocator {
    buckets: Vec<ExprStruct>,
}

impl Parse for BucketAllocator {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let parse = Punctuated::<ExprStruct, Token![,]>::parse_terminated;
        let items = parse(input)?;
        Ok(BucketAllocator {
            buckets: items.into_iter().collect(),
        })
    }
}

#[proc_macro]
pub fn bucket_allocator(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let bucket: BucketAllocator = parse_macro_input!(input);
    dbg!(bucket.buckets.len());
    // let mut sizes: Vec<String> = vec![];
    // // for i in 1..=2 {
    // let parse = Punctuated::<ExprStruct, Token![,]>::parse_terminated;
    // let items = parse(TokenStream::from(input).into()).unwrap();
    // if !item.path.is_ident("Bucket") {
    //     panic!("bucket_allocator! expects Buckets");
    // }
    // let f = item.fields.iter().nth(0).unwrap();
    // let Member::Named(name) = &f.member else {panic!("lol");};
    // let name_str = stringify!(name);
    // sizes.push(name_str.to_string());
    // // }
    // let fin: String = sizes.join("_");
    let bucket_fields: Vec<TokenStream> = bucket
        .buckets
        .iter()
        .enumerate()
        .map(|(idx, bucket)| {
            // bucket.
            quote! { "lol" }
        })
        .collect();
    quote! {
        struct BucketAllocator {

        }
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        // bucket_allocator! {
        // "hi"
        // };
    }
}
