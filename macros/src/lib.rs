use std::mem;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Error, Field, Fields, Ident, LitInt, Token, Type, WhereClause, parse::ParseStream, parse_quote, spanned::Spanned};

fn execute(exec: impl FnOnce() -> Result<TokenStream, Error>) -> proc_macro::TokenStream {
    match exec() {
        Ok(stream) => stream.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro_derive(MapAssetIds, attributes(assets))]
pub fn derive_map_asset_ids(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    execute(|| {
        fn destructure_fields(fields: &Fields) -> TokenStream {
            match fields {
                Fields::Named(fields) => {
                    let names = fields.named.iter().flat_map(|f| f.ident.as_ref());
                    quote! { { #(#names),* } }
                }
                Fields::Unnamed(fields) => {
                    let names = fields
                        .unnamed
                        .iter()
                        .enumerate()
                        .map(|(i, f)| Ident::new(&format!("field_{i}"), f.span()));
                    quote! { (#(#names),*) }
                }
                Fields::Unit => TokenStream::new(),
            }
        }

        fn asset_field(field_index: usize, field: &Field, where_clause: &mut WhereClause) -> Result<Option<TokenStream>, Error> {
            let mut found = false;
            for attr in &field.attrs {
                if attr.path().is_ident("assets") && mem::replace(&mut found, true) {
                    Err(Error::new_spanned(field, "Duplicated `#[assets]` attribute"))?
                }
            }

            Ok(found.then(|| {
                let ty = &field.ty;
                where_clause.predicates.push(parse_quote! { #ty: crate::MapAssetIds });
                match &field.ident {
                    Some(id) => quote! { #id.map_asset_ids(mapper); },
                    None => {
                        let id = Ident::new(&format!("field_{field_index}"), field.span());
                        quote! { #id.map_asset_ids(mapper); }
                    }
                }
            }))
        }

        let mut derive = syn::parse::<DeriveInput>(input)?;
        let name = derive.ident;

        let where_clause = derive.generics.make_where_clause();
        let data = match derive.data {
            Data::Struct(data_struct) => {
                let destructured = destructure_fields(&data_struct.fields);
                let mut data = vec![quote! {
                    let Self #destructured = self;
                }];

                for (field_index, field) in data_struct.fields.iter().enumerate() {
                    if let Some(f) = asset_field(field_index, field, where_clause)? {
                        data.push(f);
                    }
                }
                quote! { #(#data)* }
            }
            Data::Enum(data_enum) => {
                let mut data = Vec::new();
                for variant in data_enum.variants {
                    let variant_name = variant.ident;
                    let destructured = destructure_fields(&variant.fields);

                    let mut inner_data = Vec::new();
                    for (field_index, field) in variant.fields.iter().enumerate() {
                        if let Some(f) = asset_field(field_index, field, where_clause)? {
                            inner_data.push(f);
                        }
                    }

                    data.push(quote! {
                        Self::#variant_name #destructured => {
                            #(#inner_data)*
                        }
                    });
                }

                quote! {
                    match self {
                        #(#data)*
                    }
                }
            }
            Data::Union(..) => Err(Error::new_spanned(&name, "Unions are not supported"))?,
        };

        let (impl_generics, type_generics, where_clause) = derive.generics.split_for_impl();
        Ok(quote! {
            impl #impl_generics crate::MapAssetIds for #name #type_generics #where_clause {
                #[allow(unused, reason = "Automatic implementation of `MapAssetIds` enumerates over all fields")]
                fn map_asset_ids(&mut self, mapper: &mut dyn crate::AssetIdMapper) {
                    #data
                }
            }
        })
    })
}

#[proc_macro_derive(Save, attributes(version))]
pub fn derive_save(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    execute(|| {
        let mut derive = syn::parse::<DeriveInput>(input)?;
        let name = derive.ident;

        let mut version = None;
        for attr in &derive.attrs {
            if attr.path().is_ident("version") && version.replace(attr).is_some() {
                Err(Error::new_spanned(attr, "Duplicated `#[version(...)]` attribute"))?
            }
        }

        let version = version.ok_or_else(|| Error::new_spanned(&name, "Missing `#[version(...)]` attribute"))?;
        let versions = version.parse_args_with(|attr: ParseStream| {
            attr.parse_terminated(
                |stream: ParseStream| {
                    let version = stream.parse::<LitInt>()?;
                    stream.parse::<Token![=]>()?;
                    let repr = stream.parse::<Type>()?;
                    Ok((version.base10_parse::<u32>()?, repr))
                },
                Token![,],
            )
        })?;

        let (last_version, last_repr) = versions
            .last()
            .ok_or_else(|| Error::new_spanned(version, "Expected at least one version in `#[version(...)]` attribute"))?;

        let where_clause = derive.generics.make_where_clause();
        where_clause.predicates.push(parse_quote! {
            Self: ::bevy::reflect::Reflectable + ::serde::ser::Serialize + for<'de> ::serde::de::Deserialize<'de>
        });
        where_clause.predicates.push(parse_quote! {
            Self: crate::saves::SaveSpec::<#last_version, Repr = Self>
        });

        let mut impl_specs = vec![quote! {
            impl crate::saves::SaveSpec::<#last_version> for #name {
                type Repr = #last_repr;
            }
        }];

        let first_version = versions.first().expect("Iterator content already checked above").0;
        let mut impl_loaders = vec![quote! {
            let loader = crate::saves::Loader::<Self, #first_version>::new();
        }];

        for ((from_version, from_repr), (to_version, to_repr)) in versions.iter().zip(versions.iter().skip(1)) {
            where_clause.predicates.push(parse_quote! {
                #to_repr: ::core::convert::From<#from_repr>
            });
            impl_specs.push(quote! {
                impl crate::saves::SaveSpec::<#from_version> for #name {
                    type Repr = #from_repr;
                }
            });
            impl_loaders.push(quote! {
                let loader = loader.next::<#to_version>(<#to_repr as ::core::convert::From::<#from_repr>>::from);
            });
        }

        let (impl_generics, type_generics, where_clause) = derive.generics.split_for_impl();
        Ok(quote! {
            impl #impl_generics crate::saves::Save for #name #type_generics #where_clause {
                fn saver() -> crate::saves::SaverWithInput::<Self> {
                    crate::saves::Saver::<Self, #last_version>::new().finish()
                }

                fn loader() -> crate::saves::LoaderWithOutput::<Self> {
                    #(#impl_loaders)*
                    loader.finish()
                }
            }

            #(#impl_specs)*
        })
    })
}
