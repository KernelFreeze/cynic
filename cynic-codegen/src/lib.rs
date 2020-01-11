extern crate proc_macro;
use proc_macro2::TokenStream;
use std::collections::HashSet;

mod argument_struct;
mod field_selector;
mod field_type;
mod graphql_enum;
mod graphql_extensions;
mod ident;
mod input_struct;
mod module;
mod selector_struct;
mod struct_field;
mod type_path;

use argument_struct::ArgumentStruct;
use graphql_enum::GraphQLEnum;
use graphql_extensions::FieldExt;
use input_struct::InputStruct;
use module::Module;
use selector_struct::SelectorStruct;

#[derive(Debug, PartialEq)]
enum Error {
    IoError(String),
    ParseError(String),
}

impl From<graphql_parser::schema::ParseError> for Error {
    fn from(e: graphql_parser::schema::ParseError) -> Error {
        Error::ParseError(e.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::IoError(e.to_string())
    }
}

#[derive(Debug)]
struct QueryDslParams {
    schema_filename: String,
}

impl QueryDslParams {
    fn new(schema_filename: String) -> Self {
        QueryDslParams { schema_filename }
    }
}

impl syn::parse::Parse for QueryDslParams {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        input
            .parse::<syn::LitStr>()
            .map(|lit_str| QueryDslParams::new(lit_str.value()))
    }
}

#[proc_macro]
pub fn query_dsl(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as QueryDslParams);

    query_dsl_from_schema(input).unwrap().into()
}

fn query_dsl_from_schema(input: QueryDslParams) -> Result<TokenStream, Error> {
    use quote::quote;

    let schema = std::fs::read_to_string(&input.schema_filename)?;
    let schema_data: GraphQLSchema = graphql_parser::schema::parse_schema(&schema)?.into();

    Ok(quote! {
        #schema_data
    })
}

#[derive(Debug)]
struct GraphQLSchema {
    selectors: Vec<SelectorStruct>,
    enums: Vec<GraphQLEnum>,
    argument_struct_modules: Vec<Module<ArgumentStruct>>,
    inputs: Vec<InputStruct>,
}

impl From<graphql_parser::schema::Document> for GraphQLSchema {
    fn from(document: graphql_parser::schema::Document) -> Self {
        use graphql_parser::schema::{Definition, TypeDefinition};

        let mut scalar_names = HashSet::new();

        for definition in &document.definitions {
            match definition {
                Definition::TypeDefinition(TypeDefinition::Scalar(scalar)) => {
                    scalar_names.insert(scalar.name.clone());
                }
                _ => {}
            }
        }

        let mut selectors = vec![];
        let mut enums = vec![];
        let mut argument_struct_modules = vec![];
        let mut inputs = vec![];

        for definition in document.definitions {
            match definition {
                Definition::TypeDefinition(TypeDefinition::Object(object)) => {
                    selectors.push(SelectorStruct::from_object(&object, &scalar_names));

                    let mut argument_structs = vec![];
                    for field in &object.fields {
                        let required_arguments = field.required_arguments();
                        if !required_arguments.is_empty() {
                            argument_structs.push(ArgumentStruct::from_field(
                                field,
                                true,
                                &scalar_names,
                            ));
                        }

                        let optional_arguments = field.optional_arguments();
                        if !optional_arguments.is_empty() {
                            argument_structs.push(ArgumentStruct::from_field(
                                field,
                                false,
                                &scalar_names,
                            ));
                        }
                    }

                    if !argument_structs.is_empty() {
                        argument_struct_modules.push(Module::new(&object.name, argument_structs));
                    }
                }
                Definition::TypeDefinition(TypeDefinition::Enum(gql_enum)) => {
                    enums.push(gql_enum.into());
                }
                Definition::TypeDefinition(TypeDefinition::InputObject(obj)) => {
                    inputs.push(InputStruct::from_input_object(obj, &scalar_names));
                }
                _ => {}
            }
        }

        GraphQLSchema {
            selectors,
            enums,
            argument_struct_modules,
            inputs,
        }
    }
}

impl quote::ToTokens for GraphQLSchema {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use quote::{quote, TokenStreamExt};

        let enums = &self.enums;
        let selectors = &self.selectors;
        let argument_struct_modules = &self.argument_struct_modules;
        let inputs = &self.inputs;

        tokens.append_all(quote! {
            #(
                #enums
            )*
            #(
                #selectors
            )*
            #(
                #argument_struct_modules
            )*
            #(
                #inputs
            )*
        })
    }
}
