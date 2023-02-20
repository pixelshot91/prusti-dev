window.SIDEBAR_ITEMS = {"enum":[["AddBounds","Changes how bounds are added"],["BindStyle","The type of binding to use when generating a pattern."]],"fn":[["unpretty_print","Dumps an unpretty version of a tokenstream. Takes any type which implements `Display`."]],"struct":[["BindingInfo","Information about a specific binding. This contains both an `Ident` reference to the given field, and the syn `&'a Field` descriptor for that field."],["Structure","A wrapper around a `syn::DeriveInput` which provides utilities for creating custom derive trait implementations."],["VariantAst","This type is similar to `syn`’s `Variant` type, however each of the fields are references rather than owned. When this is used as the AST for a real variant, this struct simply borrows the fields of the `syn::Variant`, however this type may also be used as the sole variant for a struct."],["VariantInfo","A wrapper around a `syn::DeriveInput`’s variant which provides utilities for destructuring `Variant`s with `match` expressions."]],"trait":[["MacroResult","Helper trait describing values which may be returned by macro implementation methods used by this crate’s macros."]]};