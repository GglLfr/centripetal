use proc_macro2::TokenStream;
use syn::Error;

#[expect(unused, reason = "No proc macros yet")]
fn execute(exec: impl FnOnce() -> Result<TokenStream, Error>) -> proc_macro::TokenStream {
    match exec() {
        Ok(stream) => stream.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
