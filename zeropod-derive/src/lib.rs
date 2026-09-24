//! `#[derive(ZeroPod)]`: Borsh's encoding for a struct or an enum, and a
//! view that reads and writes it in place.
//!
//! The derive only calls the field types' `ZeroPod` implementations, in
//! declaration order; the encoding, its checks and its `unsafe` live there.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Attribute, Data, DeriveInput, Error, Expr, Fields, Ident, Path, Result, Token, Type,
    Visibility, parse_macro_input, punctuated::Punctuated,
};

#[proc_macro_derive(ZeroPod, attributes(zeropod, max_len))]
pub fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

fn expand(input: &DeriveInput) -> Result<TokenStream> {
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &input.generics,
            "zeropod types cannot be generic",
        ));
    }
    let krate = krate(&input.attrs)?;
    match &input.data {
        Data::Struct(data) => {
            let Fields::Named(_) = &data.fields else {
                return Err(Error::new_spanned(
                    input,
                    "a view names its fields: use named fields",
                ));
            };
            Ok(structure(input, &fields(&data.fields)?, &krate))
        }
        Data::Enum(data) => enumeration(input, data, &krate),
        Data::Union(_) => Err(Error::new_spanned(
            input,
            "zeropod stores structs and enums",
        )),
    }
}

/// `#[zeropod(crate = path)]`, for a crate that re-exports zeropod.
fn krate(attrs: &[Attribute]) -> Result<TokenStream> {
    let mut krate = quote!(::zeropod);
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("zeropod")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("crate") {
                let path: Path = meta.value()?.parse()?;
                krate = quote!(#path);
                Ok(())
            } else {
                Err(meta.error("expected `crate = path`"))
            }
        })?;
    }
    Ok(krate)
}

/// A field: its name (or `f0`.. in a tuple variant), type, visibility, and
/// `#[max_len(..)]` bounds as a slice.
struct Field {
    name: Ident,
    ty: Type,
    vis: Visibility,
    limits: TokenStream,
}

fn fields(fields: &Fields) -> Result<Vec<Field>> {
    fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let mut limits = Vec::new();
            for attr in field
                .attrs
                .iter()
                .filter(|attr| attr.path().is_ident("max_len"))
            {
                let bounds =
                    attr.parse_args_with(Punctuated::<Expr, Token![,]>::parse_terminated)?;
                limits.extend(bounds);
            }
            Ok(Field {
                name: field
                    .ident
                    .clone()
                    .unwrap_or_else(|| format_ident!("f{index}")),
                ty: field.ty.clone(),
                vis: field.vis.clone(),
                limits: quote!(&[#(#limits),*]),
            })
        })
        .collect()
}

/// The offset of the field after `before`, walking them from `start`.
/// Uses `bytes`; call inside `unsafe`, on a valid encoding.
fn offset(before: &[Field], start: TokenStream, krate: &TokenStream) -> TokenStream {
    let tys = before.iter().map(|field| &field.ty);
    quote! {{
        let mut at: usize = #start;
        #(at += <#tys as #krate::ZeroPod>::len(#krate::__private::from_unchecked(bytes, at));)*
        at
    }}
}

fn structure(input: &DeriveInput, fields: &[Field], krate: &TokenStream) -> TokenStream {
    let (name, vis) = (&input.ident, &input.vis);
    let view = format_ident!("{name}View");
    let names: Vec<&Ident> = fields.iter().map(|field| &field.name).collect();
    let tys: Vec<&Type> = fields.iter().map(|field| &field.ty).collect();
    let limits: Vec<&TokenStream> = fields.iter().map(|field| &field.limits).collect();
    let zp = quote!(#krate::ZeroPod);
    let private = quote!(#krate::__private);
    let sizes = quote!(&[#(<#tys as #zp>::SIZE),*]);
    let any_bytes = quote!(true #(&& <#tys as #zp>::ANY_BYTES)*);

    let order = fields.iter().enumerate().map(|(index, field)| {
        let message = format!(
            "`{}` has a fixed size, so it comes before the fields whose size varies",
            field.name
        );
        quote! {
            const _: () = ::core::assert!(!#private::fixed_after_variable(#sizes, #index), #message);
        }
    });

    let accessors = fields.iter().enumerate().map(|(index, field)| {
        let Field {
            name: field_name,
            ty,
            vis,
            limits,
        } = field;
        let setter = format_ident!("set_{field_name}");
        let at = offset(&fields[..index], quote!(0), krate);
        let doc_get = format!("`{field_name}`, read in place.");
        let doc_set =
            format!("Writes `{field_name}`, moving the fields after it within the view's room.");
        quote! {
            #[doc = #doc_get]
            #vis fn #field_name(&self) -> <#ty as #zp>::Ref<'_> {
                let bytes = self.0.as_slice();
                // SAFETY: the view holds a valid encoding, whose fields
                // before this one end where this one starts.
                unsafe { <#ty as #zp>::read(#private::from_unchecked(bytes, #at)) }
            }

            #[doc = #doc_set]
            #vis fn #setter(
                &mut self,
                value: <#ty as #zp>::In<'_>,
            ) -> ::core::result::Result<(), #krate::Error> {
                let total = <#name as #krate::Layout>::size(self);
                let bytes = self.0.as_slice();
                // SAFETY: as in the getter.
                let at = unsafe { #at };
                // SAFETY: the view holds a valid encoding `total` long, in
                // which this field starts at `at`.
                unsafe { #private::replace::<#ty>(&mut self.0, at, total, &value, #limits) }
            }
        }
    });
    let size = (!names.iter().any(|name| *name == "size")).then(|| {
        quote! {
            /// The bytes the encoding takes, without the room after it.
            #vis fn size(&self) -> usize {
                <#name as #krate::Layout>::size(self)
            }
        }
    });
    let view_doc = format!("A `{name}` read and written in place.");

    quote! {
        #[doc = #view_doc]
        #[repr(transparent)]
        #vis struct #view(#krate::Bytes);

        impl ::core::convert::AsRef<#krate::Bytes> for #view {
            fn as_ref(&self) -> &#krate::Bytes {
                &self.0
            }
        }

        #(#order)*

        // SAFETY: each method visits the fields in order, through their own
        // implementations, which agree with one another.
        unsafe impl #zp for #name {
            type Ref<'a> = &'a #view;
            type In<'a> = &'a #name;
            const SIZE: ::core::option::Option<usize> = #private::sum(#sizes);
            const ANY_BYTES: bool = #any_bytes;

            fn check(
                bytes: &[u8],
                _: &[usize],
            ) -> ::core::result::Result<usize, #krate::Error> {
                let mut at: usize = 0;
                #(at += <#tys as #zp>::check(#private::from(bytes, at), #limits)?;)*
                ::core::result::Result::Ok(at)
            }

            unsafe fn len(bytes: &[u8]) -> usize {
                // SAFETY: forwarded: a valid encoding is its fields in order.
                unsafe { #private::len_of::<Self>(bytes, |bytes| {
                    let mut at: usize = 0;
                    #(at += <#tys as #zp>::len(#private::from_unchecked(bytes, at));)*
                    at
                }) }
            }

            unsafe fn read(bytes: &[u8]) -> &#view {
                // SAFETY: the view is a transparent `Bytes`, which is a
                // transparent `[u8]`, and the bytes hold a valid encoding.
                unsafe { &*(bytes as *const [u8] as *const #view) }
            }

            fn encoded_len(
                value: &&#name,
                _: &[usize],
            ) -> ::core::result::Result<usize, #krate::Error> {
                let mut len: usize = 0;
                #(len += <#tys as #zp>::encoded_len(&#zp::input(&value.#names), #limits)?;)*
                ::core::result::Result::Ok(len)
            }

            unsafe fn write(value: &&#name, out: &mut [u8]) -> usize {
                let mut at: usize = 0;
                // SAFETY: `out` holds every field, in order.
                unsafe {
                    #(at += <#tys as #zp>::write(
                        &#zp::input(&value.#names),
                        #private::from_unchecked_mut(out, at),
                    );)*
                }
                at
            }

            fn max_len(_: &[usize]) -> ::core::option::Option<usize> {
                let mut len: usize = 0;
                #(len = len.checked_add(<#tys as #zp>::max_len(#limits)?)?;)*
                ::core::option::Option::Some(len)
            }

            fn input(&self) -> &#name {
                self
            }

            unsafe fn own(view: &#view) -> #name {
                // SAFETY: forwarded: each field came from its `read`.
                unsafe { #name { #(#names: <#tys as #zp>::own(view.#names()),)* } }
            }
        }

        // SAFETY: the view is a transparent `Bytes`, `as_ref` returns it, and
        // both casts are to it.
        unsafe impl #krate::Layout for #name {
            type View = #view;

            unsafe fn view_unchecked(bytes: &[u8]) -> &#view {
                // SAFETY: forwarded.
                unsafe { <#name as #zp>::read(bytes) }
            }

            unsafe fn view_unchecked_mut(bytes: &mut [u8]) -> &mut #view {
                // SAFETY: as in `read`.
                unsafe { &mut *(bytes as *mut [u8] as *mut #view) }
            }
        }

        impl #view {
            #(#accessors)*
            #size
        }
    }
}

fn enumeration(
    input: &DeriveInput,
    data: &syn::DataEnum,
    krate: &TokenStream,
) -> Result<TokenStream> {
    let (name, vis) = (&input.ident, &input.vis);
    let reference = format_ident!("{name}Ref");
    let zp = quote!(#krate::ZeroPod);
    let private = quote!(#krate::__private);

    // Borsh's tag: the variant's index, or its discriminant when written.
    let mut tags = Vec::new();
    let mut next: u16 = 0;
    for variant in &data.variants {
        if let Some((_, discriminant)) = &variant.discriminant {
            let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Int(int),
                ..
            }) = discriminant
            else {
                return Err(Error::new_spanned(
                    discriminant,
                    "a tag is an integer literal",
                ));
            };
            next = int.base10_parse()?;
        }
        let tag = u8::try_from(next)
            .map_err(|_| Error::new_spanned(&variant.ident, "a tag is at most 255"))?;
        tags.push(tag);
        next += 1;
    }

    let variants: Vec<(&Ident, &Fields, Vec<Field>)> = data
        .variants
        .iter()
        .map(|variant| Ok((&variant.ident, &variant.fields, fields(&variant.fields)?)))
        .collect::<Result<_>>()?;

    // Binds a variant's fields by name, of `name` or of `reference`.
    let pattern = |ty: &Ident, variant: &Ident, shape: &Fields, fields: &[Field]| {
        let names = fields.iter().map(|field| &field.name);
        match shape {
            Fields::Named(_) => quote!(#ty::#variant { #(#names),* }),
            Fields::Unnamed(_) => quote!(#ty::#variant(#(#names),*)),
            Fields::Unit => quote!(#ty::#variant),
        }
    };

    let ref_variants = variants.iter().map(|(variant, shape, fields)| {
        let tys = fields.iter().map(|field| &field.ty);
        let names = fields.iter().map(|field| &field.name);
        match shape {
            Fields::Named(_) => quote!(#variant { #(#names: <#tys as #zp>::Ref<'a>),* }),
            Fields::Unnamed(_) => quote!(#variant(#(<#tys as #zp>::Ref<'a>),*)),
            Fields::Unit => quote!(#variant),
        }
    });

    let variant_sizes = variants.iter().map(|(_, _, fields)| {
        let tys = fields.iter().map(|field| &field.ty);
        quote!(#private::sum(&[#(<#tys as #zp>::SIZE),*]))
    });

    let checks = variants.iter().zip(&tags).map(|((_, _, fields), tag)| {
        let (tys, limits) = (
            fields.iter().map(|f| &f.ty),
            fields.iter().map(|f| &f.limits),
        );
        quote! {
            ::core::option::Option::Some(#tag) => {
                #(at += <#tys as #zp>::check(#private::from(bytes, at), #limits)?;)*
            }
        }
    });

    let lens = variants.iter().zip(&tags).map(|((_, _, fields), tag)| {
        let at = offset(fields, quote!(1), krate);
        quote!(#tag => #at,)
    });

    let reads = variants
        .iter()
        .zip(&tags)
        .map(|((variant, shape, fields), tag)| {
            let bound = fields.iter().enumerate().map(|(index, field)| {
                let (field_name, ty) = (&field.name, &field.ty);
                let at = offset(&fields[..index], quote!(1), krate);
                quote!(let #field_name = <#ty as #zp>::read(#private::from_unchecked(bytes, #at));)
            });
            let value = pattern(&reference, variant, shape, fields);
            quote!(#tag => { #(#bound)* #value })
        });

    let encoded_lens = variants.iter().map(|(variant, shape, fields)| {
        let (tys, limits) = (
            fields.iter().map(|f| &f.ty),
            fields.iter().map(|f| &f.limits),
        );
        let names = fields.iter().map(|field| &field.name);
        let value = pattern(name, variant, shape, fields);
        quote! {
            #value => {
                let mut len: usize = 1;
                #(len += <#tys as #zp>::encoded_len(&#zp::input(#names), #limits)?;)*
                len
            }
        }
    });

    let writes = variants
        .iter()
        .zip(&tags)
        .map(|((variant, shape, fields), tag)| {
            let tys = fields.iter().map(|field| &field.ty);
            let names = fields.iter().map(|field| &field.name);
            let value = pattern(name, variant, shape, fields);
            quote! {
                #value => {
                    *out.get_unchecked_mut(0) = #tag;
                    let mut at: usize = 1;
                    #(at += <#tys as #zp>::write(
                        &#zp::input(#names),
                        #private::from_unchecked_mut(out, at),
                    );)*
                    at
                }
            }
        });

    let max_lens = variants.iter().map(|(_, _, fields)| {
        let (tys, limits) = (
            fields.iter().map(|f| &f.ty),
            fields.iter().map(|f| &f.limits),
        );
        quote! {{
            let mut len: usize = 1;
            #(len = len.checked_add(<#tys as #zp>::max_len(#limits)?)?;)*
            ::core::option::Option::Some(len)
        }}
    });

    let owns = variants.iter().map(|(variant, shape, fields)| {
        let from = pattern(&reference, variant, shape, fields);
        let owned = fields.iter().map(|field| {
            let (field_name, ty) = (&field.name, &field.ty);
            quote!(<#ty as #zp>::own(#field_name))
        });
        let names = fields.iter().map(|field| &field.name);
        let to = match shape {
            Fields::Named(_) => quote!(#name::#variant { #(#names: #owned),* }),
            Fields::Unnamed(_) => quote!(#name::#variant(#(#owned),*)),
            Fields::Unit => quote!(#name::#variant),
        };
        quote!(#from => #to,)
    });
    let reference_doc = format!("A `{name}` read in place.");

    Ok(quote! {
        #[doc = #reference_doc]
        ///
        /// Match it by value: its hidden variant holds no value, so the match
        /// needs no arm for it.
        #vis enum #reference<'a> {
            #(#ref_variants,)*
            /// Uses the lifetime, whatever the fields; it has no value.
            #[doc(hidden)]
            __Borrow(::core::marker::PhantomData<&'a ()>, ::core::convert::Infallible),
        }

        // SAFETY: each method reads the tag, then visits that variant's
        // fields in order, through their own implementations.
        unsafe impl #zp for #name {
            type Ref<'a> = #reference<'a>;
            type In<'a> = &'a #name;
            const SIZE: ::core::option::Option<usize> =
                match #private::same(&[#(#variant_sizes),*]) {
                    ::core::option::Option::Some(size) => ::core::option::Option::Some(1 + size),
                    ::core::option::Option::None => ::core::option::Option::None,
                };

            fn check(
                bytes: &[u8],
                _: &[usize],
            ) -> ::core::result::Result<usize, #krate::Error> {
                let mut at: usize = 1;
                match bytes.first() {
                    #(#checks)*
                    ::core::option::Option::Some(_) => {
                        return ::core::result::Result::Err(#krate::Error::InvalidTag);
                    }
                    ::core::option::Option::None => {
                        return ::core::result::Result::Err(#krate::Error::TooShort);
                    }
                }
                ::core::result::Result::Ok(at)
            }

            unsafe fn len(bytes: &[u8]) -> usize {
                // SAFETY: a valid encoding starts with a known tag, then
                // that variant's fields.
                unsafe { #private::len_of::<Self>(bytes, |bytes| match *bytes.get_unchecked(0) {
                    #(#lens)*
                    _ => ::core::hint::unreachable_unchecked(),
                }) }
            }

            unsafe fn read(bytes: &[u8]) -> #reference<'_> {
                // SAFETY: as in `len`.
                unsafe {
                    match *bytes.get_unchecked(0) {
                        #(#reads)*
                        _ => ::core::hint::unreachable_unchecked(),
                    }
                }
            }

            fn encoded_len(
                value: &&#name,
                _: &[usize],
            ) -> ::core::result::Result<usize, #krate::Error> {
                ::core::result::Result::Ok(match *value {
                    #(#encoded_lens)*
                })
            }

            unsafe fn write(value: &&#name, out: &mut [u8]) -> usize {
                // SAFETY: `out` holds the tag and every field of the variant.
                unsafe {
                    match *value {
                        #(#writes)*
                    }
                }
            }

            fn max_len(_: &[usize]) -> ::core::option::Option<usize> {
                let mut len: usize = 0;
                #(len = len.max(#max_lens?);)*
                ::core::option::Option::Some(len)
            }

            fn input(&self) -> &#name {
                self
            }

            unsafe fn own(value: #reference<'_>) -> #name {
                // SAFETY: forwarded: each field came from its `read`.
                unsafe {
                    match value {
                        #(#owns)*
                        #reference::__Borrow(_, never) => match never {},
                    }
                }
            }
        }
    })
}
