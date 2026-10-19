//! The `#[justifies("REQ001", ...)]` attribute, which marks a function as evidence
//! that the named requirements are fulfilled.
//!
//! The attribute does not change the function. `emperor check` finds it by reading
//! the source and treats the function as a justification of each named requirement.

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn justifies(_requirements: TokenStream, function: TokenStream) -> TokenStream {
    function
}
