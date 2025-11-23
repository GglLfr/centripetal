use std::mem;

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
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
        fn asset_fields<'a>(
            fields: impl IntoIterator<Item = &'a Field>,
            where_clause: &mut WhereClause,
        ) -> impl Iterator<Item = Result<TokenStream, Error>> {
            fields.into_iter().enumerate().filter_map(|(i, f)| {
                let mut found = false;
                for attr in &f.attrs {
                    if attr.path().is_ident("assets") && mem::replace(&mut found, true) {
                        return Some(Err(Error::new_spanned(f, "Duplicated `#[assets]` attribute")))
                    }
                }

                found.then(|| {
                    let ty = &f.ty;
                    where_clause.predicates.push(parse_quote! { #ty: crate::MapAssetIds });

                    Ok(match &f.ident {
                        Some(id) => id.to_token_stream(),
                        None => Ident::new(&format!("field_{i}"), f.span()).to_token_stream(),
                    })
                })
            })
        }

        fn visit_fields(fields: &Fields, where_clause: &mut WhereClause) -> Result<(TokenStream, Vec<TokenStream>), Error> {
            let mut field_names = Vec::new();
            let destructured = match fields {
                Fields::Named(fields) => {
                    for f in asset_fields(&fields.named, where_clause) {
                        field_names.push(f?);
                    }
                    quote! { { #(#field_names,)* .. } }
                }
                Fields::Unnamed(fields) => {
                    for f in asset_fields(&fields.unnamed, where_clause) {
                        field_names.push(f?);
                    }
                    quote! { (#(#field_names,)* ..) }
                }
                Fields::Unit => TokenStream::new(),
            };

            Ok((destructured, field_names))
        }

        let mut derive = syn::parse::<DeriveInput>(input)?;
        let name = derive.ident;

        let where_clause = derive.generics.make_where_clause();
        let (visit, map) = match derive.data {
            Data::Struct(data) => {
                let (destructure, fields) = visit_fields(&data.fields, where_clause)?;
                (
                    quote! {
                        let Self #destructure = self;
                        #(#fields.visit_asset_ids(visitor);)*
                    },
                    quote! {
                        let Self #destructure = self;
                        #(#fields.map_asset_ids(mapper);)*
                    },
                )
            }
            Data::Enum(data) => {
                let mut variants = Vec::new();
                let mut destructures = Vec::new();
                let mut fields = Vec::new();

                for variant in data.variants {
                    let (destructure, field) = visit_fields(&variant.fields, where_clause)?;
                    variants.push(variant.ident);
                    destructures.push(destructure);
                    fields.push(field);
                }

                (
                    quote! {
                        match self {
                            #(
                                Self::#variants #destructures => {
                                    #(#fields.visit_asset_ids(visitor);)*
                                }
                            )*
                        }
                    },
                    quote! {
                        match self {
                            #(
                                Self::#variants #destructures => {
                                    #(#fields.map_asset_ids(visitor);)*
                                }
                            )*
                        }
                    },
                )
            }

            Data::Union(..) => Err(Error::new_spanned(&name, "Unions are not supported"))?,
        };

        let (impl_generics, type_generics, where_clause) = derive.generics.split_for_impl();
        Ok(quote! {
            impl #impl_generics crate::MapAssetIds for #name #type_generics #where_clause {
                fn visit_asset_ids(&self, visitor: &mut dyn ::core::ops::FnMut(::bevy::asset::UntypedAssetId)) {
                    #visit
                }

                fn map_asset_ids(&mut self, mapper: &mut dyn crate::AssetIdMapper) {
                    #map
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
