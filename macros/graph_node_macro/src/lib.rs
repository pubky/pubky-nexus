extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

#[proc_macro_derive(GraphNode)]
pub fn graph_node_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let fields = match input.data {
        Data::Struct(data_struct) => match data_struct.fields {
            Fields::Named(fields_named) => fields_named.named,
            _ => unimplemented!(),
        },
        _ => unimplemented!(),
    };

    let field_initializers = fields.iter().map(|field| {
        let field_name = &field.ident;
        let field_name_str = field_name.as_ref().unwrap().to_string();
        quote! {
            #field_name: node.get(#field_name_str).unwrap_or_default()
        }
    });

    let expanded = quote! {
        impl #name {
            pub fn from_node(node: &Node) -> Self {
                #name {
                    #(#field_initializers,)*
                }
            }
        }
    };

    TokenStream::from(expanded)
}
