use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::parse::{Parse, ParseStream, Parser, Result};
use syn::{
    parse_macro_input, parse_quote, Field, Fields, FieldsNamed, Ident, ItemStruct, Token, Type,
};

struct RegistryArgs {
    pub(crate) pairs: Vec<(Ident, Type)>,
}

impl Parse for RegistryArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut pairs = vec![];
        while !input.is_empty() {
            let name: Ident = input.parse()?;
            input.parse::<Token![:]>()?;
            let ty: Type = input.parse()?;
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
            pairs.push((name, ty));
        }
        Ok(Self { pairs })
    }
}

struct RegistryStruct {
    pub(crate) item: ItemStruct,
}

impl RegistryStruct {
    fn add_fields(&mut self, new_fields: Vec<TokenStream>) {
        if let Fields::Named(ref mut fields) = self.item.fields {
            // if there are already some fields
            for field in new_fields {
                fields.named.push(Field::parse_named.parse2(field).unwrap());
            }
        } else {
            // there are no fields
            let fields: FieldsNamed = parse_quote! {
                {
                    #(#new_fields),*
                }
            };
            self.item.fields = Fields::Named(fields);
        }
    }
}

impl Parse for RegistryStruct {
    fn parse(input: ParseStream) -> Result<Self> {
        let item = input.parse::<ItemStruct>()?;
        Ok(Self { item })
    }
}

pub fn make_registry(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(input as RegistryStruct);
    let (existing_fields, existing_fields_assignment) =
        if let Fields::Named(ref mut fields) = input.item.fields {
            let pairs: Vec<_> = fields
                .named
                .iter()
                .map(|field| {
                    let (ident, ty) = (field.ident.clone(), field.ty.clone());
                    quote! {
                        #ident: #ty
                    }
                })
                .collect();
            let names: Vec<_> = fields
                .named
                .iter()
                .map(|field| field.ident.to_token_stream())
                .collect();
            (pairs, names)
        } else {
            (vec![], vec![])
        };

    let args = parse_macro_input!(args as RegistryArgs);

    let channel_idents: Vec<_> = args
        .pairs
        .iter()
        .map(|(name, ty)| {
            let tx = Ident::new(&format!("{}_tx", name), Span::call_site());
            let rx = Ident::new(&format!("{}_rx", name), Span::call_site());
            (name, ty, tx, rx)
        })
        .collect();

    let make_channels: Vec<_> = channel_idents
        .iter()
        .map(|(_name, ty, tx, rx)| {
            quote! {
                let (#tx, mut #rx) = tokio::sync::mpsc::channel::<(<#ty as genserver::GenServer>::Message, Option<tokio::sync::oneshot::Sender<<#ty as genserver::GenServer>::Response>>)>(1_000);
            }
        })
        .collect();

    let tx_channels: Vec<_> = channel_idents
        .iter()
        .map(|(_name, ty, tx, _rx)| {
            quote! {
                #tx: tokio::sync::mpsc::Sender<(<#ty as genserver::GenServer>::Message, Option<tokio::sync::oneshot::Sender<<#ty as genserver::GenServer>::Response>>)>
            }
        })
        .collect();

    let assign_tx_channels: Vec<_> = channel_idents
        .iter()
        .map(|(_name, _ty, tx, _rx)| {
            quote! {
                #tx
            }
        })
        .collect();

    let handlers: Vec<_> = channel_idents
        .iter()
        .map(|(name, ty, tx, _rx)| {
            let call_fn = Ident::new(&format!("call_{}", name), Span::call_site());
            let cast_fn = Ident::new(&format!("cast_{}", name), Span::call_site());
            quote! {
                pub async fn #call_fn(&self, message: <#ty as genserver::GenServer>::Message) -> Result<<#ty as genserver::GenServer>::Response, genserver::Error<(<#ty as genserver::GenServer>::Message, Option<tokio::sync::oneshot::Sender<<#ty as genserver::GenServer>::Message>>)>> {
                    let (oneshot_tx, oneshot_rx) = tokio::sync::oneshot::channel::<<#ty as genserver::GenServer>::Response>();
                    self.#tx.send((message, Some(oneshot_tx))).await?;
                    let Response = oneshot_rx.await?;
                    Ok(Response)
                }
                pub async fn #cast_fn(&self, message: <#ty as genserver::GenServer>::Message) -> Result<(), genserver::Error<(<#ty as genserver::GenServer>::Message, Option<tokio::sync::oneshot::Sender<<#ty as genserver::GenServer>::Message>>)>> {
                    self.#tx.send((message, None)).await?;
                    Ok(())
                }
            }
        })
        .collect();

    let start_servers: Vec<_> = channel_idents
        .iter()
        .map(|(_name, ty, _tx, rx)| {
            quote! {
                {
                    let local_registry = registry.clone();
                    tokio::spawn(async move {
                        let mut handler = #ty::new(local_registry);
                        while let Some((message, oneshot)) = #rx.recv().await {
                            if let Some(oneshot) = oneshot {
                                let Response = handler.handle_call(message).await;
                                oneshot.send(Response).ok();
                            } else {
                                handler.handle_cast(message).await;
                            }
                        }
                    });
                }
            }
        })
        .collect();

    input.add_fields(tx_channels);

    let ident = input.item.ident.clone();
    let registry = input.item.clone();

    let (impl_generics, ty_generics, where_clause) = input.item.generics.split_for_impl();
    let impl_trait_block = quote! {
        impl #impl_generics genserver::Registry for #ident #ty_generics #where_clause {}
    };

    let impl_block = quote! {
        impl #impl_generics #ident #ty_generics #where_clause {
            pub async fn start(
                #(#existing_fields),*
            ) -> Self {
                #(#make_channels)*
                let mut registry = Self {
                    #(#existing_fields_assignment,)*
                    #(#assign_tx_channels,)*
                };
                #(#start_servers)*
                registry
            }
            #(#handlers)*
        }
    };

    let registry = quote! {
        #[derive(Clone)]
        #registry
        #impl_trait_block
        #impl_block
    };

    registry.into()
}
