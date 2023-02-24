extern crate proc_macro;
extern crate quote;
extern crate syn;

use proc_macro::TokenStream;
use proc_macro2::{Ident, Literal, Span};
use quote::quote;
use std::collections::HashSet;
use std::iter::FromIterator;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Lit, Token};

#[derive(Debug)]
struct TestInput {
    test_folder: String,
    known_test_failures: HashSet<String>,
    initial_known_test_failures: HashSet<String>,
}

impl Parse for TestInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        // Expect the input to be bracketed
        syn::bracketed!(content in input);
        // Expect the content of the brackets to be literals separated by ","
        let inner_tokens: Punctuated<Lit, Token![,]> = content.parse_terminated(Lit::parse)?;
        // Convert the tokens into strings
        let inner_strings: Vec<String> = inner_tokens
            .iter()
            .filter_map(|s| match s {
                Lit::Str(a) => Some(a.value()),
                _ => {
                    println!("Warning: ignoring non-string literal in input list.");
                    None
                }
            })
            .collect();
        // The first string is the test directory, the others are the tests that are known to fail
        Ok(TestInput {
            test_folder: inner_strings.first().expect("test directory").clone(),
            known_test_failures: inner_strings.clone().into_iter().skip(1).collect(),
            initial_known_test_failures: inner_strings.into_iter().skip(1).collect(),
        })
    }
}

/// Generates the integration tests for the validator.
/// The expected input is a list with the location of the folder containing the binary of the problems
/// followed by the name of the tests that are known to fail.
///
/// # Example
/// Assuming that the folder `bins/problems/` contains the files `problem0[1-3].bin` and we know that
/// the problems 02 and 03 fail.
///
/// ```
/// generate_tests!(["planning/ext/up/bins/problems/", "problem02", "problem03"]);
/// ```
/// The macro call above will generate the following code:
/// ```
/// #[test]
/// fn test_problem01() {
///     assert!(common::valid_plan("problem01").is_ok());
/// }
///
/// #[test]
/// fn test_problem02_fails() {
///     assert!(common::valid_plan("problem02").is_err());
/// }
///
/// #[test]
/// fn test_problem03_fails() {
///     assert!(common::valid_plan("problem03").is_err());
/// }
/// ```
#[proc_macro]
pub fn generate_tests(input: TokenStream) -> TokenStream {
    // Parse the input in our local struct
    let mut test_input = syn::parse_macro_input!(input as TestInput);

    // Get all file names (without the extension) contained in the test folder.
    let entries: Vec<String> = std::fs::read_dir(test_input.test_folder.clone())
        .expect("dir")
        .map(|res| res.map(|e| e.path()))
        .map(|p| {
            if p.is_err() {
                panic!("A PathBuf is wrapped in an error.")
            }
            let pathbuf = p.expect("pathbuf");
            let filename = pathbuf.file_name().expect("filename").to_str().expect("str");
            let extension_position = filename.rfind('.').expect("extension");
            let filename = &filename[..extension_position];
            if test_input.known_test_failures.contains(filename) {
                // Remove in order to detect when a test is expected to fail but it is not present in the test folder
                test_input.known_test_failures.remove(filename);
            }
            filename.to_string()
        })
        .collect();

    // At least one test is expected to fail but was not present in the test folder
    if !test_input.known_test_failures.is_empty() {
        panic!(
            "One or more KTFs didn't match an actual test file: {:?}",
            test_input.known_test_failures
        )
    }

    // Generate the test functions
    let mut streams: Vec<TokenStream> = vec![];
    entries.iter().for_each(|test_filename| {
        let mut result = "is_ok";
        let mut test_name = "test_".to_owned();
        test_name.push_str(&test_filename.replace('-', "_"));
        let filename = Literal::string(test_filename);

        if test_input.initial_known_test_failures.contains(test_filename) {
            test_name.push_str("_fails");
            result = "is_err";
        }
        let methodname = Ident::new(&test_name, Span::call_site());
        let result = Ident::new(result, Span::call_site());

        streams.push(
            (quote! {
                #[test]
                fn #methodname() {
                    assert!(common::valid_plan(#filename).#result());
                }
            })
            .into(),
        );
    });
    TokenStream::from_iter(streams.into_iter())
}
