use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse::{self, Parser},
    parse_macro_input, Expr, ExprAssign, ItemStruct,
};

#[proc_macro_attribute]
pub fn b2_basic_body_init(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut item_struct = parse_macro_input!(input as ItemStruct);
    let _ = parse_macro_input!(args as parse::Nothing);

    if let syn::Fields::Named(ref mut fields) = item_struct.fields {
        fields.named.push(
            syn::Field::parse_named
                .parse2(quote! {
                    #[builder(setter(into))]
                    pub bucket_id: String
                })
                .unwrap(),
        );
    }

    return quote! {
        #[derive(Clone, Debug, Serialize, TypedBuilder)]
        #[serde(rename_all = "camelCase")]
        #item_struct
    }
    .into();
}

#[proc_macro_derive(IntoHeaderMap)]
pub fn impl_into_header_map(input: TokenStream) -> TokenStream {
    let mut item_struct = parse_macro_input!(input as ItemStruct);
    let mut names: Vec<(String, String)> = Vec::new();
    let struct_name: quote::__private::TokenStream = item_struct.ident.to_string().parse().unwrap();

    if let syn::Fields::Named(ref mut fields) = item_struct.fields {
        for field in fields.named.iter() {
            let field_name = field.ident.as_ref().unwrap().to_string();
            let alt_name = field.attrs.iter().find(|attr| {
                let list = match attr.meta.require_list() {
                    Ok(nv) => nv,
                    Err(_) => return false,
                };

                if list.path.get_ident().unwrap().to_string() != "serde" {
                    return false;
                }

                let argument: Expr = match list.parse_args() {
                    Ok(args) => args,
                    Err(_) => return false,
                };

                return match argument {
                    Expr::Assign(assign) => {
                        assign.left.as_ref().to_token_stream().to_string() == "rename"
                    }
                    _ => false,
                };
            });

            let header_name: String = match alt_name {
                Some(alt) => {
                    let name = alt
                        .meta
                        .require_list()
                        .unwrap()
                        .parse_args::<ExprAssign>()
                        .unwrap()
                        .right
                        .to_token_stream()
                        .to_string();

                    String::from(&name[1..name.len() - 1])
                }
                None => field_name.clone(),
            };

            names.push((header_name, field_name));
        }
    }

    let final_stream: quote::__private::TokenStream = names.iter().map(|(map_name, field_name)| {
        let field_name: quote::__private::TokenStream = field_name.parse().unwrap();
        quote! {
            insert_to_map(#map_name.into(), serde_json::to_value(self.#field_name).unwrap_or_default());
        }
    })
    .collect();

    let temp = quote! {
        impl Into<HeaderMap> for #struct_name {
            fn into(self) -> HeaderMap {
                let map_value = |value: serde_json::Value| HeaderValue::from_str(&match value {
                    serde_json::Value::Null => String::from(""),
                    serde_json::Value::String(str_value) => str_value,
                    _ => value.to_string(),
                }).unwrap();

                let mut map = HeaderMap::new();

                let mut insert_to_map = |key: &str, value: serde_json::Value| {
                    let header_value = map_value(value);
                    if !header_value.is_empty() {
                        map.insert(HeaderName::from_str(&key).unwrap(), header_value);
                    }
                };

                #final_stream
                map
            }
        }
    };

    return temp.into();
}
