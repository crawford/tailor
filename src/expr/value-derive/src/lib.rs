// Copyright 2017 CoreOS, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

extern crate proc_macro;
#[macro_use]
extern crate quote;
extern crate syn;

use proc_macro::TokenStream;

#[proc_macro_derive(Value)]
pub fn value(input: TokenStream) -> TokenStream {
    let ast = syn::parse_derive_input(&input.to_string()).unwrap();
    let name = ast.ident;

    let body = match ast.body {
        syn::Body::Struct(syn::VariantData::Struct(fields)) => {
            let inserts: Vec<quote::Tokens> = fields
                .iter()
                .map(|field| {
                    let ident = field.ident.as_ref().expect(
                        "Value cannot be derived from tuple struct",
                    );
                    quote! {
					map.insert(stringify!(#ident).into(), s.#ident.into());
				}
                })
                .collect();

            quote! {
				let mut map = ::std::collections::HashMap::new();
				#(#inserts);*
				Value::Dictionary(map)
			}
        }
        _ => panic!("Value can only be derived from a struct"),
    };

    (quote! {
		impl From<#name> for Value {
			fn from(s: #name) -> Self {
				#body
			}
		}
	}).parse()
        .unwrap()
}
