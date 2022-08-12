mod registry;

#[proc_macro_attribute]
pub fn make_registry(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    registry::make_registry(args, input)
}
