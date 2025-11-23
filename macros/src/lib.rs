use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Error, LitInt, Token, Type, parse::ParseStream, parse_quote};

fn execute(exec: impl FnOnce() -> Result<TokenStream, Error>) -> proc_macro::TokenStream {
    match exec() {
        Ok(stream) => stream.into(),
        Err(e) => e.to_compile_error().into(),
    }
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
