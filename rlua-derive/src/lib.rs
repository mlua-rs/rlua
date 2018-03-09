extern crate quote;
#[macro_use]
extern crate syn;
#[macro_use]
extern crate synstructure;

use std::slice;
use syn::{Data, DeriveInput, Fields};
use synstructure::{BindingInfo, Structure};

decl_derive!([LuaTable] => derive_lua_table);

fn derive_lua_table(mut s: synstructure::Structure) -> quote::Tokens {
    let struct_type = check_struct_type(s.ast());
    let struct_name = s.ast().ident;

    // get body for impl<'lua> IntoTable<'lua>
    let into_body = match struct_type {
        StructType::NormalStruct => s.each(|bind| {
            let name = bind.ast().ident.unwrap();
            quote! {
                table.set(stringify!(#name), self.#name)?;
            }
        }),
        StructType::TupleStruct => {
            let mut count = 0;
            s.each(|__bind| {
                count += 1;
                quote! {
                    table.set(format!("_{}", #count - 1), self.0)?;
                }
            })
        }
    };

    // get body for impl<'lua> FromTable<'lua>
    let from_body = {
        let bindings = get_binding_iter(&s);
        let body = match struct_type {
            StructType::NormalStruct => bindings.fold(quote::Tokens::new(), |mut t, bind| {
                let name = bind.ast().ident.unwrap();
                t.append_all(quote! {
                    #name: table.get(stringify!(#name))?,
                });
                t
            }),
            StructType::TupleStruct => {
                let mut count = 0;
                bindings.fold(quote::Tokens::new(), |mut t, __bind| {
                    count += 1;
                    t.append_all(quote! {
                        table.get(format!("_{}", #count - 1))?,
                    });
                    t
                })
            }
        };
        match struct_type {
            StructType::NormalStruct => quote!( { #body }),
            StructType::TupleStruct => quote!( ( #body )),
        }
    };
    s.add_impl_generic(parse_quote!('lua));
    let mut tokens = s.unbound_impl(
        quote!(::rlua::IntoTable<'lua>),
        quote! {
            fn into_table(
                self,
                lua: &'lua Lua,
            ) -> ::std::result::Result<::rlua::Table<'lua>, ::rlua::Error> {
                let table = lua.create_table()?;
                match self {
                    #into_body
                }
                Ok(table)
            }
        },
    );
    tokens.append_all(s.unbound_impl(
        quote!(::rlua::FromTable<'lua>),
        quote!{
            fn from_table(
                table: Table<'lua>,
                _lua: &'lua Lua,
            ) -> ::std::result::Result<Self, ::rlua::Error> {
                Ok(#struct_name #from_body)
            }
        },
    ));
    tokens
}

fn get_binding_iter<'a>(s: &'a Structure) -> slice::Iter<'a, BindingInfo<'a>> {
    s.variants()
        .into_iter()
        .nth(0)
        .unwrap()
        .bindings()
        .into_iter()
}

enum StructType {
    TupleStruct,
    NormalStruct,
}

fn check_struct_type(input: &DeriveInput) -> StructType {
    let unsupported = |s| panic!("{} is not supported by #[derive(IntoTable)]!", s);
    match input.data {
        Data::Struct(ref data) => match data.fields {
            Fields::Named(_) => StructType::NormalStruct,
            Fields::Unnamed(_) => StructType::TupleStruct,
            Fields::Unit => unsupported("Unit struct"),
        },
        Data::Enum(_) => unsupported("Enum"),
        Data::Union(_) => unsupported("Union"),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn normal_struct() {
        test_derive! {
            derive_lua_table {
                struct Point {
                    x: i32,
                    y: i32,
                }
            }
            expands to {
                #[allow(non_upper_case_globals)]
                const _DERIVE_rlua_IntoTable_lua_FOR_Point: () = {
                    impl<'lua> ::rlua::IntoTable<'lua> for Point {
                        fn into_table(
                            self,
                            lua: &'lua Lua,
                        ) -> ::std::result::Result<::rlua::Table<'lua>, ::rlua::Error> {
                            let table = lua.create_table()?;
                            match self {
                                Point {
                                    x: ref __binding_0,
                                    y: ref __binding_1,
                                } => {
                                    {
                                        table.set(stringify!(x), self.x)?;
                                    }
                                    {
                                        table.set(stringify!(y), self.y)?;
                                    }
                                }
                            }
                            Ok(table)
                        }
                    }
                };
                #[allow(non_upper_case_globals)]
                const _DERIVE_rlua_FromTable_lua_FOR_Point: () = {
                    impl<'lua> ::rlua::FromTable<'lua> for Point {
                        fn from_table(
                            table: Table<'lua>,
                            _lua: &'lua Lua,
                        ) -> ::std::result::Result<Self, ::rlua::Error> {
                              Ok(Point {
                                  x: table.get(stringify!(x))?,
                                  y: table.get(stringify!(y))?,
                              })
                        }
                    }
                };
            }
            no_build
        }
    }

    #[test]
    fn tuple_struct() {
        test_derive! {
            derive_lua_table {
                struct Point(i32, i32);
            }
            expands to {
                #[allow(non_upper_case_globals)]
                const _DERIVE_rlua_IntoTable_lua_FOR_Point: () = {
                    impl<'lua> ::rlua::IntoTable<'lua> for Point {
                        fn into_table(
                            self,
                            lua: &'lua Lua,
                        ) -> ::std::result::Result<::rlua::Table<'lua>, ::rlua::Error> {
                            let table = lua.create_table()?;
                            match self {
                                Point(ref __binding_0 , ref __binding_1, ) => {
                                    {
                                        table.set(format!("_{}", 1i32 - 1), self.0)?;
                                    }
                                    {
                                        table.set(format!("_{}", 2i32 - 1), self.0)?;
                                    }
                                }
                            }
                            Ok(table)
                        }
                    }
                };
                #[allow(non_upper_case_globals)]
                const _DERIVE_rlua_FromTable_lua_FOR_Point: () = {
                    impl<'lua> ::rlua::FromTable<'lua> for Point {
                        fn from_table(
                            table: Table<'lua>,
                            _lua: &'lua Lua,
                        ) -> ::std::result::Result<Self, ::rlua::Error> {
                            Ok(Point(
                                table.get(format!("_{}", 1i32 - 1))?,
                                table.get(format!("_{}", 2i32 - 1))?,
                            ))
                        }
                    }
                };
            }
            no_build
        }
    }
}
