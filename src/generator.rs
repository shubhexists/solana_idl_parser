use crate::parser::{
    Idl, IdlDefinedType, IdlEnumVariant, IdlEnumVariantFields, IdlInstruction, IdlType, IdlTypeDef,
    IdlTypeDefFields,
};
use convert_case::{Case, Casing};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub fn generate_idl_code(idl: &Idl) -> TokenStream {
    let program_name = &idl.metadata.name;
    let program_name_pascal = program_name.to_case(Case::Pascal);
    let enum_name = format_ident!("{}Instructions", program_name_pascal);
    let discriminators = generate_discriminators(&idl.instructions);
    let instruction_structs = generate_instruction_structs(&idl.instructions);
    let types = generate_types(&idl.types);
    let instructions_enum = generate_instructions_enum(&enum_name, &idl.instructions);
    let deserialize_impl = generate_deserialize_impl(&enum_name, &idl.instructions);

    quote! {
        #discriminators
        #instruction_structs
        #types
        #instructions_enum
        #deserialize_impl
    }
}

fn generate_discriminators(instructions: &[IdlInstruction]) -> TokenStream {
    let discriminators = instructions.iter().map(|ix| {
        let name = ix.name.to_case(Case::ScreamingSnake);
        let const_name = format_ident!("{}_DISCRIMINATOR", name);
        let bytes: Vec<u8> = ix.discriminator.clone();
        let byte_literals = bytes.iter().map(|b| quote! { #b });

        quote! {
            pub const #const_name: [u8; 8] = [#(#byte_literals),*];
        }
    });

    quote! { #(#discriminators)* }
}

fn generate_instruction_structs(instructions: &[IdlInstruction]) -> TokenStream {
    let structs = instructions.iter().map(|ix| {
        let name_pascal = ix.name.to_case(Case::Pascal);
        let name_screaming = ix.name.to_case(Case::ScreamingSnake);

        let mut tokens = TokenStream::new();

        if !ix.accounts.is_empty() {
            let accounts_len = ix.accounts.len();
            let len_const = format_ident!("{}_IX_ACCOUNTS_LEN", name_screaming);
            let accounts_struct_name = format_ident!("{}Accounts", name_pascal);

            let account_fields = ix.accounts.iter().map(|acc| {
                let field_name = format_ident!("{}", acc.name.to_case(Case::Snake));
                quote! { pub #field_name: ::solana_sdk::pubkey::Pubkey }
            });

            let from_metas_fields = ix.accounts.iter().enumerate().map(|(i, acc)| {
                let field_name = format_ident!("{}", acc.name.to_case(Case::Snake));
                let idx = syn::Index::from(i);
                quote! { #field_name: metas[#idx].pubkey }
            });

            tokens.extend(quote! {
                pub const #len_const: usize = #accounts_len;

                #[derive(Copy, Clone, Debug, PartialEq, ::borsh::BorshDeserialize, ::borsh::BorshSerialize)]
                pub struct #accounts_struct_name {
                    #(#account_fields,)*
                }

                impl #accounts_struct_name {
                    pub fn from_account_metas(metas: &[::solana_program::instruction::AccountMeta]) -> ::anyhow::Result<Self> {
                        if metas.len() != #len_const {
                            return Err(::std::io::Error::new(
                                ::std::io::ErrorKind::Other,
                                "invalid account meta length",
                            ).into());
                        }
                        Ok(Self {
                            #(#from_metas_fields,)*
                        })
                    }
                }
            });
        }

        if !ix.args.is_empty() {
            let args_struct_name = format_ident!("{}Args", name_pascal);
            let arg_fields = ix.args.iter().map(|arg| {
                let field_name = format_ident!("{}", arg.name.to_case(Case::Snake));
                let field_type = idl_type_to_rust(&arg.ty);
                quote! { pub #field_name: #field_type }
            });

            tokens.extend(quote! {
                #[derive(Debug, ::borsh::BorshDeserialize, ::borsh::BorshSerialize)]
                pub struct #args_struct_name {
                    #(#arg_fields,)*
                }
            });
        }

        tokens
    });

    quote! { #(#structs)* }
}

fn generate_types(types: &[IdlTypeDef]) -> TokenStream {
    let type_defs = types.iter().map(|typedef| {
        let name = format_ident!("{}", typedef.name);

        match typedef.ty.kind.as_str() {
            "struct" => match &typedef.ty.fields {
                IdlTypeDefFields::Named(fields) => {
                    let field_defs = fields.iter().map(|f| {
                        let field_name = format_ident!("{}", f.name.to_case(Case::Snake));
                        let field_type = idl_type_to_rust(&f.ty);
                        quote! { pub #field_name: #field_type }
                    });

                    quote! {
                        #[derive(Debug, Clone, ::borsh::BorshDeserialize, ::borsh::BorshSerialize)]
                        pub struct #name {
                            #(#field_defs,)*
                        }
                    }
                }
                IdlTypeDefFields::Tuple(types) => {
                    let field_types = types.iter().map(|ty| {
                        let field_type = idl_type_to_rust(ty);
                        quote! { pub #field_type }
                    });

                    quote! {
                        #[derive(Debug, Clone, ::borsh::BorshDeserialize, ::borsh::BorshSerialize)]
                        pub struct #name(#(#field_types),*);
                    }
                }
                IdlTypeDefFields::None => {
                    quote! {
                        #[derive(Debug, Clone, ::borsh::BorshDeserialize, ::borsh::BorshSerialize)]
                        pub struct #name;
                    }
                }
            },
            "enum" => {
                let variants = typedef.ty.variants.iter().map(|v| generate_enum_variant(v));

                quote! {
                    #[derive(Debug, Clone, ::borsh::BorshDeserialize, ::borsh::BorshSerialize)]
                    pub enum #name {
                        #(#variants,)*
                    }
                }
            }
            _ => quote! {},
        }
    });

    quote! { #(#type_defs)* }
}

fn generate_enum_variant(variant: &IdlEnumVariant) -> TokenStream {
    let name = format_ident!("{}", variant.name);

    match &variant.fields {
        Some(IdlEnumVariantFields::Named(fields)) => {
            let field_defs = fields.iter().map(|f| {
                let field_name = format_ident!("{}", f.name.to_case(Case::Snake));
                let field_type = idl_type_to_rust(&f.ty);
                quote! { #field_name: #field_type }
            });
            quote! { #name { #(#field_defs,)* } }
        }
        Some(IdlEnumVariantFields::Tuple(types)) => {
            let field_types = types.iter().map(|ty| idl_type_to_rust(ty));
            quote! { #name(#(#field_types,)*) }
        }
        None => quote! { #name },
    }
}

fn generate_instructions_enum(
    enum_name: &syn::Ident,
    instructions: &[IdlInstruction],
) -> TokenStream {
    let variants = instructions.iter().map(|ix| {
        let variant_name = format_ident!("{}", ix.name.to_case(Case::Pascal));
        let name_pascal = ix.name.to_case(Case::Pascal);

        let has_accounts = !ix.accounts.is_empty();
        let has_args = !ix.args.is_empty();

        match (has_accounts, has_args) {
            (true, true) => {
                let accounts_type = format_ident!("{}Accounts", name_pascal);
                let args_type = format_ident!("{}Args", name_pascal);
                quote! { #variant_name(#accounts_type, #args_type) }
            }
            (true, false) => {
                let accounts_type = format_ident!("{}Accounts", name_pascal);
                quote! { #variant_name(#accounts_type) }
            }
            (false, true) => {
                let args_type = format_ident!("{}Args", name_pascal);
                quote! { #variant_name(#args_type) }
            }
            (false, false) => {
                quote! { #variant_name }
            }
        }
    });

    quote! {
        #[derive(Debug, ::borsh::BorshDeserialize, ::borsh::BorshSerialize)]
        pub enum #enum_name {
            #(#variants,)*
        }
    }
}

fn generate_deserialize_impl(
    enum_name: &syn::Ident,
    instructions: &[IdlInstruction],
) -> TokenStream {
    let match_arms = instructions.iter().map(|ix| {
        let name_screaming = ix.name.to_case(Case::ScreamingSnake);
        let variant_name = format_ident!("{}", ix.name.to_case(Case::Pascal));
        let discrim_const = format_ident!("{}_DISCRIMINATOR", name_screaming);
        let name_pascal = ix.name.to_case(Case::Pascal);

        let has_accounts = !ix.accounts.is_empty();
        let has_args = !ix.args.is_empty();

        match (has_accounts, has_args) {
            (true, true) => {
                let accounts_type = format_ident!("{}Accounts", name_pascal);
                let args_type = format_ident!("{}Args", name_pascal);
                quote! {
                    #discrim_const => Ok(Self::#variant_name(
                        #accounts_type::from_account_metas(&accounts)?,
                        #args_type::deserialize(&mut reader)?,
                    ))
                }
            }
            (true, false) => {
                let accounts_type = format_ident!("{}Accounts", name_pascal);
                quote! {
                    #discrim_const => Ok(Self::#variant_name(
                        #accounts_type::from_account_metas(&accounts)?,
                    ))
                }
            }
            (false, true) => {
                let args_type = format_ident!("{}Args", name_pascal);
                quote! {
                    #discrim_const => Ok(Self::#variant_name(
                        #args_type::deserialize(&mut reader)?,
                    ))
                }
            }
            (false, false) => {
                quote! {
                    #discrim_const => Ok(Self::#variant_name)
                }
            }
        }
    });

    quote! {
        impl #enum_name {
            pub fn deserialize(accounts: ::std::vec::Vec<::solana_program::instruction::AccountMeta>, buf: &[u8]) -> ::anyhow::Result<Self> {
                use ::std::io::Read as _;
                use ::borsh::BorshDeserialize as _;
                let mut reader = buf;
                let mut maybe_discm = [0u8; 8];
                reader.read_exact(&mut maybe_discm)?;

                match maybe_discm {
                    #(#match_arms,)*
                    _ => Err(::std::io::Error::new(
                        ::std::io::ErrorKind::Other,
                        "unknown discriminator"
                    ).into())
                }
            }
        }
    }
}

fn idl_type_to_rust(ty: &IdlType) -> TokenStream {
    match ty {
        IdlType::Primitive(s) => match s.as_str() {
            "bool" => quote! { bool },
            "u8" => quote! { u8 },
            "u16" => quote! { u16 },
            "u32" => quote! { u32 },
            "u64" => quote! { u64 },
            "u128" => quote! { u128 },
            "i8" => quote! { i8 },
            "i16" => quote! { i16 },
            "i32" => quote! { i32 },
            "i64" => quote! { i64 },
            "i128" => quote! { i128 },
            "f32" => quote! { f32 },
            "f64" => quote! { f64 },
            "string" => quote! { String },
            "pubkey" => quote! { ::solana_sdk::pubkey::Pubkey },
            "bytes" => quote! { Vec<u8> },
            other => {
                let ident = format_ident!("{}", other);
                quote! { #ident }
            }
        },
        IdlType::Defined { defined } => {
            let name = match defined {
                IdlDefinedType::Simple(s) => s.clone(),
                IdlDefinedType::Named { name } => name.clone(),
            };
            let ident = format_ident!("{}", name);
            quote! { #ident }
        }
        IdlType::Option { option } => {
            let inner = idl_type_to_rust(option);
            quote! { Option<#inner> }
        }
        IdlType::Vec { vec } => {
            let inner = idl_type_to_rust(vec);
            quote! { Vec<#inner> }
        }
        IdlType::Array { array } => {
            let (inner, size) = array;
            let inner_type = idl_type_to_rust(inner);
            quote! { [#inner_type; #size] }
        }
    }
}
