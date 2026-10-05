//! `#[derive(ZeroPod)]`: Borsh's encoding for a struct or an enum, and a
//! view that reads and writes it in place.
//!
//! The derive only calls the field types' `ZeroPod` implementations, in
//! declaration order; the encoding, its checks and its `unsafe` live there.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Attribute, Data, DataEnum, DeriveInput, Error, Expr, Fields, GenericParam, Generics, Ident,
    Index, Member, Path, Result, Token, Type, Visibility,
    ext::IdentExt,
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
    visit_mut::{self, VisitMut},
};

#[proc_macro_derive(ZeroPod, attributes(zeropod, borsh, max_len))]
pub fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// What every expansion needs: paths into zeropod, and the type's generics
/// with each type parameter bound by `ZeroPod`.
struct Context {
    krate: TokenStream,
    zp: TokenStream,
    private: TokenStream,
    generics: Generics,
}

fn expand(input: &DeriveInput) -> Result<TokenStream> {
    if let Some(lifetime) = input.generics.lifetimes().next() {
        return Err(Error::new_spanned(
            lifetime,
            "zeropod stores owned values: no lifetimes",
        ));
    }
    let options = Options::parse(&input.attrs)?;
    let krate = options.krate;
    let mut generics = input.generics.clone();
    let params: Vec<Ident> = generics
        .type_params()
        .map(|param| param.ident.clone())
        .collect();
    let where_clause = generics.make_where_clause();
    for param in params {
        where_clause
            .predicates
            .push(parse_quote!(#param: #krate::ZeroPod));
    }
    let context = Context {
        zp: quote!(#krate::ZeroPod),
        private: quote!(#krate::__private),
        krate,
        generics,
    };
    match &input.data {
        Data::Struct(data) => {
            let fields = fields(&data.fields, input)?;
            match (&data.fields, fields.as_slice()) {
                (Fields::Unnamed(_), [field]) if !field.skip => Ok(newtype(input, field, &context)),
                _ => Ok(structure(input, &fields, &context)),
            }
        }
        Data::Enum(data) => enumeration(input, data, options.use_discriminant, &context),
        Data::Union(_) => Err(Error::new_spanned(
            input,
            "zeropod stores structs and enums",
        )),
    }
}

/// `#[zeropod(crate = path)]`, for a crate that re-exports zeropod, and
/// `#[borsh(use_discriminant = ..)]`, as Borsh reads it.
struct Options {
    krate: TokenStream,
    use_discriminant: Option<bool>,
}

impl Options {
    fn parse(attrs: &[Attribute]) -> Result<Self> {
        let mut options = Options {
            krate: quote!(::zeropod),
            use_discriminant: None,
        };
        for attr in attrs {
            if attr.path().is_ident("zeropod") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("crate") {
                        let path: Path = meta.value()?.parse()?;
                        options.krate = quote!(#path);
                        Ok(())
                    } else {
                        Err(meta.error("expected `crate = path`"))
                    }
                })?;
            } else if attr.path().is_ident("borsh") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("use_discriminant") {
                        let value: syn::LitBool = meta.value()?.parse()?;
                        options.use_discriminant = Some(value.value);
                    } else if meta.path.is_ident("crate") {
                        // Borsh's own path: nothing to zeropod.
                        meta.value()?.parse::<Expr>()?;
                    } else {
                        return Err(meta.error("zeropod reads `use_discriminant` and `crate` here"));
                    }
                    Ok(())
                })?;
            }
        }
        Ok(options)
    }
}

/// A field: how the value reaches it (`name` or `0`), what its methods and
/// bindings are called, and its `#[max_len(..)]` bounds as a slice.
struct Field {
    member: Member,
    name: Ident,
    ty: Type,
    vis: Visibility,
    limits: TokenStream,
    bounded: bool,
    /// `#[borsh(skip)]` or `#[zeropod(skip)]`: not stored, `Default` when read.
    skip: bool,
}

/// Resolve Self in bound expressions before emitting them in views/editors.
struct BoundOwner(Path);

impl VisitMut for BoundOwner {
    fn visit_path_mut(&mut self, path: &mut Path) {
        visit_mut::visit_path_mut(self, path);
        if path
            .segments
            .first()
            .is_some_and(|segment| segment.ident == "Self")
        {
            let mut resolved = self.0.clone();
            resolved
                .segments
                .extend(path.segments.iter().skip(1).cloned());
            *path = resolved;
        }
    }
}

fn fields(fields: &Fields, input: &DeriveInput) -> Result<Vec<Field>> {
    let name = &input.ident;
    let (_, args, _) = input.generics.split_for_impl();
    let mut owner: Path = parse_quote!(#name #args);
    if let syn::PathArguments::AngleBracketed(args) =
        &mut owner.segments.last_mut().unwrap().arguments
    {
        args.colon2_token = Some(Default::default());
    }
    let mut owner = BoundOwner(owner);
    fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let mut limits = Vec::new();
            let mut skip = false;
            for attr in &field.attrs {
                if attr.path().is_ident("max_len") {
                    let bounds = Punctuated::<Expr, Token![,]>::parse_terminated;
                    limits.extend(attr.parse_args_with(bounds)?);
                } else if attr.path().is_ident("borsh") || attr.path().is_ident("zeropod") {
                    attr.parse_nested_meta(|meta| {
                        if meta.path.is_ident("skip") {
                            skip = true;
                            Ok(())
                        } else {
                            Err(meta.error("zeropod reads `skip` here"))
                        }
                    })?;
                }
            }
            let (member, name) = match &field.ident {
                Some(ident) => (Member::Named(ident.clone()), ident.clone()),
                None => (
                    Member::Unnamed(Index::from(index)),
                    format_ident!("_{index}"),
                ),
            };
            for limit in &mut limits {
                owner.visit_expr_mut(limit);
            }
            Ok(Field {
                member,
                name,
                ty: field.ty.clone(),
                vis: field.vis.clone(),
                bounded: !limits.is_empty(),
                limits: quote!(&[#(#limits),*]),
                skip,
            })
        })
        .collect()
}

/// The offset of the field after `before`, walking them from `start`.
/// Uses `bytes`; call inside `unsafe`, on a valid encoding.
fn offset(before: &[&Field], start: TokenStream, context: &Context) -> TokenStream {
    let Context { zp, private, .. } = context;
    let tys = before.iter().map(|field| &field.ty);
    quote! {{
        let mut at: usize = #start;
        #(at += <#tys as #zp>::len(#private::from_unchecked(bytes, at));)*
        at
    }}
}

/// `SIZE`: the fields' sizes summed, after asserting, for a struct, that the
/// fixed ones come first, so each has an offset known when compiling.
fn size(stored: &[&Field], ordered: bool, context: &Context) -> TokenStream {
    let Context { zp, private, .. } = context;
    let tys: Vec<&Type> = stored.iter().map(|field| &field.ty).collect();
    let sizes = quote!(&[#(<#tys as #zp>::SIZE),*]);
    let order = stored
        .iter()
        .enumerate()
        .filter(|_| ordered)
        .map(|(index, field)| {
            let message = format!(
                "`{}` has a fixed size, so it comes before the fields whose size varies",
                field.name
            );
            quote!(::core::assert!(!#private::fixed_after_variable(#sizes, #index), #message);)
        });
    quote!({ #(#order)* #private::sum(#sizes) })
}

/// A struct of one unnamed field: stored as that field, and read as it.
/// `#[repr(transparent)]` makes its bytes in memory the field's, so it is
/// `NATIVE` when the field is.
fn newtype(input: &DeriveInput, field: &Field, context: &Context) -> TokenStream {
    let Context {
        krate,
        zp,
        generics,
        ..
    } = context;
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let ty = &field.ty;
    let limits = if field.bounded {
        field.limits.clone()
    } else {
        quote!(_limits)
    };
    let transparent = input.attrs.iter().any(|attr| {
        let mut transparent = false;
        if attr.path().is_ident("repr") {
            let _ = attr.parse_nested_meta(|meta| {
                transparent |= meta.path.is_ident("transparent");
                Ok(())
            });
        }
        transparent
    });
    quote! {
        // SAFETY: every method is the field's.
        unsafe impl #impl_generics #zp for #name #ty_generics #where_clause {
            type Ref<'a> = <#ty as #zp>::Ref<'a> where Self: 'a;
            type In<'a> = <#ty as #zp>::In<'a> where Self: 'a;
            const SIZE: ::core::option::Option<usize> = <#ty as #zp>::SIZE;
            const ANY_BYTES: bool = <#ty as #zp>::ANY_BYTES;
            const NATIVE: bool = #transparent && <#ty as #zp>::NATIVE;

            fn check(bytes: &[u8], _limits: &[usize]) -> ::core::result::Result<usize, #krate::Error> {
                <#ty as #zp>::check(bytes, #limits)
            }

            unsafe fn len(bytes: &[u8]) -> usize {
                // SAFETY: forwarded.
                unsafe { <#ty as #zp>::len(bytes) }
            }

            unsafe fn read(bytes: &[u8]) -> Self::Ref<'_> {
                // SAFETY: forwarded.
                unsafe { <#ty as #zp>::read(bytes) }
            }

            fn encoded_len(
                value: &Self::In<'_>,
                _limits: &[usize],
            ) -> ::core::result::Result<usize, #krate::Error> {
                <#ty as #zp>::encoded_len(value, #limits)
            }

            unsafe fn write(value: &Self::In<'_>, out: &mut [u8]) -> usize {
                // SAFETY: forwarded.
                unsafe { <#ty as #zp>::write(value, out) }
            }


            #[inline]
            fn max_len(_limits: &[usize]) -> ::core::option::Option<usize> {
                <#ty as #zp>::max_len(#limits)
            }

            fn input(&self) -> Self::In<'_> {
                #zp::input(&self.0)
            }

            fn own(value: Self::Ref<'_>) -> Self {
                Self(<#ty as #zp>::own(value))
            }
        }
    }
}

fn structure(input: &DeriveInput, fields: &[Field], context: &Context) -> TokenStream {
    let Context {
        krate,
        zp,
        private,
        generics,
    } = context;
    let (name, vis) = (&input.ident, &input.vis);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let view = format_ident!("{name}View");
    let editor = format_ident!("{name}Edit");
    let mut editor_generics = generics.clone();
    editor_generics.params.insert(0, parse_quote!('__zeropod));
    let (editor_impl, editor_args, editor_where) = editor_generics.split_for_impl();

    let stored: Vec<&Field> = fields.iter().filter(|field| !field.skip).collect();
    let members: Vec<&Member> = stored.iter().map(|field| &field.member).collect();
    let names: Vec<&Ident> = stored.iter().map(|field| &field.name).collect();
    let tys: Vec<&Type> = stored.iter().map(|field| &field.ty).collect();
    let limits: Vec<&TokenStream> = stored.iter().map(|field| &field.limits).collect();
    let size = size(&stored, true, context);

    let accessors = stored.iter().enumerate().map(|(index, field)| {
        let Field {
            name: field_name,
            ty,
            vis,
            limits,
            ..
        } = field;
        let setter = match &field.member {
            Member::Named(ident) => format_ident!("set_{}", ident),
            Member::Unnamed(index) => format_ident!("set_{}", index.index),
        };
        let at = offset(&stored[..index], quote!(0), context);
        let doc_get = format!("`{field_name}`, read in place.");
        let doc_set =
            format!("Writes `{field_name}`, moving the fields after it within the view's room.");
        quote! {
            #[doc = #doc_get]
            #vis fn #field_name(&self) -> <#ty as #zp>::Ref<'_> {
                let bytes = self.1.as_slice();
                // SAFETY: the view holds a valid encoding, whose fields
                // before this one end where this one starts.
                unsafe { <#ty as #zp>::read(#private::from_unchecked(bytes, #at)) }
            }

            #[doc = #doc_set]
            // Preserve field underscores without warning on names like `set__x`.
            #[allow(non_snake_case)]
            #vis fn #setter(
                &mut self,
                value: <#ty as #zp>::In<'_>,
            ) -> ::core::result::Result<(), #krate::Error> {
                let total = <#name #ty_generics as #krate::Layout>::size(self);
                let bytes = self.1.as_slice();
                // SAFETY: as in the getter.
                let at = unsafe { #at };
                // SAFETY: the view holds a valid encoding `total` long, in
                // which this field starts at `at`.
                unsafe { #private::replace::<#ty>(&mut self.1, at, || total, &value, #limits) }
            }
        }
    });
    let projections: Vec<_> = stored
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let Field {
                name,
                ty,
                vis,
                limits,
                ..
            } = field;
            let at = offset(&stored[..index], quote!(0), context);
            quote! {
                /// An exclusive field projection, retaining the enclosing buffer.
                #vis fn #name(self) -> #krate::Edit<'__zeropod, #ty> {
                    let bytes = self.0.as_bytes();
                    // SAFETY: a validated struct's fields have these offsets and
                    // bounds; replacing one preserves every enclosing encoding.
                    let at = unsafe { #at };
                    // SAFETY: the field at this offset has its declared bounds.
                    unsafe { self.0.project::<#ty>(at, |with| with(#limits)) }
                }
            }
        })
        .collect();
    let has_field = |name: &str| names.iter().any(|field| field.unraw() == name);
    let edit_method = (!has_field("edit")).then(|| {
        quote! {
            /// An exclusive editor retaining the complete enclosing encoding.
            #vis fn edit(&mut self) -> #krate::Edit<'_, #name #ty_generics> {
                // SAFETY: this view owns the complete validated encoding.
                unsafe { #krate::Edit::new(&mut self.1) }
            }
        }
    });
    let own_method = (!has_field("to_owned")).then(|| {
        quote! {
            /// The value, owned.
            #vis fn to_owned(&self) -> #name #ty_generics {
                <#name #ty_generics as #zp>::own(self)
            }
        }
    });
    let size_method = (!has_field("size")).then(|| {
        quote! {
            /// The bytes the encoding takes, without the room after it.
            #vis fn size(&self) -> usize {
                <#name #ty_generics as #krate::Layout>::size(self)
            }
        }
    });
    let values = fields.iter().map(|field| {
        let (member, name, ty) = (&field.member, &field.name, &field.ty);
        match field.skip {
            true => quote!(#member: ::core::default::Default::default()),
            false => quote!(#member: <#ty as #zp>::own(view.#name())),
        }
    });
    let view_doc = format!("A `{name}` read and written in place.");
    let view_generics = &input.generics;
    // A generic type's layout is checked where it is used; any other's on
    // every build, `cargo check` too.
    let order_check = input
        .generics
        .params
        .is_empty()
        .then(|| quote!(const _: ::core::option::Option<usize> = <#name as #zp>::SIZE;));

    quote! {
        #[doc = #view_doc]
        #[repr(transparent)]
        #vis struct #view #view_generics (
            ::core::marker::PhantomData<fn() -> #name #ty_generics>,
            #krate::Bytes,
        ) #where_clause;

        impl #impl_generics ::core::convert::AsRef<#krate::Bytes> for #view #ty_generics #where_clause {
            fn as_ref(&self) -> &#krate::Bytes {
                &self.1
            }
        }

        /// Mutable field projections retaining the complete enclosing encoding.
        #vis struct #editor #editor_generics (#krate::Edit<'__zeropod, #name #ty_generics>) #editor_where;

        impl #editor_impl #editor #editor_args #editor_where {
            #(#projections)*
        }

        impl #impl_generics #krate::EditFields for #name #ty_generics #where_clause {
            type Fields<'__zeropod> = #editor #editor_args where Self: '__zeropod;

            fn fields<'a>(edit: #krate::Edit<'a, Self>) -> Self::Fields<'a> where Self: 'a {
                #editor(edit)
            }
        }

        #order_check

        // SAFETY: each method visits the stored fields in order, through
        // their own implementations, which agree with one another.
        unsafe impl #impl_generics #zp for #name #ty_generics #where_clause {
            type Ref<'a> = &'a #view #ty_generics where Self: 'a;
            type In<'a> = &'a Self where Self: 'a;
            const SIZE: ::core::option::Option<usize> = #size;
            const ANY_BYTES: bool = true #(&& <#tys as #zp>::ANY_BYTES)*;

            fn check(
                bytes: &[u8],
                _: &[usize],
            ) -> ::core::result::Result<usize, #krate::Error> {
                // Evaluated here, so its layout checks run for every type.
                if let ::core::option::Option::Some(size) = <Self as #zp>::SIZE {
                    // Every byte pattern of the one length is valid.
                    if <Self as #zp>::ANY_BYTES {
                        return if bytes.len() >= size {
                            ::core::result::Result::Ok(size)
                        } else {
                            ::core::result::Result::Err(#krate::Error::TooShort)
                        };
                    }
                }
                let mut at: usize = 0;
                #(at += <#tys as #zp>::check(#private::from(bytes, at), #limits)?;)*
                ::core::result::Result::Ok(at)
            }

            unsafe fn len(bytes: &[u8]) -> usize {
                // SAFETY: forwarded: a valid encoding is its fields in order.
                unsafe {
                    #private::len_of::<Self>(bytes, |bytes| {
                        let mut at: usize = 0;
                        #(at += <#tys as #zp>::len(#private::from_unchecked(bytes, at));)*
                        at
                    })
                }
            }

            unsafe fn read(bytes: &[u8]) -> &#view #ty_generics {
                // SAFETY: the view is transparent over `Bytes`, which is
                // transparent over `[u8]`, and the bytes hold a valid encoding.
                unsafe { &*(bytes as *const [u8] as *const #view #ty_generics) }
            }

            fn encoded_len(
                value: &&Self,
                _: &[usize],
            ) -> ::core::result::Result<usize, #krate::Error> {
                let mut len: usize = 0;
                #(len += <#tys as #zp>::encoded_len(&#zp::input(&value.#members), #limits)?;)*
                ::core::result::Result::Ok(len)
            }

            unsafe fn write(value: &&Self, out: &mut [u8]) -> usize {
                let mut at: usize = 0;
                // SAFETY: `out` holds every field, in order.
                unsafe {
                    #(at += <#tys as #zp>::write(
                        &#zp::input(&value.#members),
                        #private::from_unchecked_mut(out, at),
                    );)*
                }
                at
            }


            #[inline]
            fn max_len(_: &[usize]) -> ::core::option::Option<usize> {
                let mut len: usize = 0;
                #(len = len.checked_add(<#tys as #zp>::max_len(#limits)?)?;)*
                ::core::option::Option::Some(len)
            }

            fn input(&self) -> &Self {
                self
            }

            fn own(view: &#view #ty_generics) -> Self {
                Self { #(#values,)* }
            }
        }

        // SAFETY: the view is transparent over `Bytes`, `as_ref` returns it,
        // and both casts are to it.
        unsafe impl #impl_generics #krate::Layout for #name #ty_generics #where_clause {
            type View = #view #ty_generics;

            unsafe fn view_unchecked(bytes: &[u8]) -> &#view #ty_generics {
                // SAFETY: forwarded.
                unsafe { <Self as #zp>::read(bytes) }
            }

            unsafe fn view_unchecked_mut(bytes: &mut [u8]) -> &mut #view #ty_generics {
                // SAFETY: as in `read`.
                unsafe { &mut *(bytes as *mut [u8] as *mut #view #ty_generics) }
            }
        }

        impl #impl_generics #view #ty_generics #where_clause {
            #(#accessors)*
            #edit_method
            #size_method
            #own_method
        }
    }
}

/// An enum's variant: its name, shape and fields.
struct Variant<'a> {
    ident: &'a Ident,
    shape: &'a Fields,
    fields: Vec<Field>,
}

fn enumeration(
    input: &DeriveInput,
    data: &DataEnum,
    use_discriminant: Option<bool>,
    context: &Context,
) -> Result<TokenStream> {
    let Context {
        krate,
        zp,
        private,
        generics,
    } = context;
    let (name, vis) = (&input.ident, &input.vis);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let reference = format_ident!("{name}Ref");

    // Borsh's tag: the variant's index, or its written discriminant unless
    // `use_discriminant = false`.
    let mut tags = Vec::new();
    let mut next: u16 = 0;
    for variant in &data.variants {
        let written = variant
            .discriminant
            .as_ref()
            .filter(|_| use_discriminant != Some(false));
        if let Some((_, discriminant)) = written {
            let Expr::Lit(syn::ExprLit {
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

    let variants: Vec<Variant> = data
        .variants
        .iter()
        .map(|variant| {
            let fields = fields(&variant.fields, input)?;
            if let Some(field) = fields.iter().find(|field| field.skip) {
                return Err(Error::new_spanned(
                    &field.ty,
                    "skip a struct's field, not a variant's",
                ));
            }
            Ok(Variant {
                ident: &variant.ident,
                shape: &variant.fields,
                fields,
            })
        })
        .collect::<Result<_>>()?;

    // Keep user field names out of local bindings: they may be `len`, `out`,
    // `bytes`, or any other name used by the generated implementation.
    let binding = |index: usize| {
        format_ident!(
            "__zeropod_field_{index}",
            span = proc_macro2::Span::mixed_site()
        )
    };
    // Binds a variant's fields, of `ty`: the enum or its reference.
    let pattern = |ty: &Ident, variant: &Variant| {
        let Variant {
            ident,
            shape,
            fields,
        } = variant;
        let bound = fields.iter().enumerate().map(|(index, field)| {
            let member = &field.member;
            let local = binding(index);
            quote!(#member: #local)
        });
        match shape {
            Fields::Unit => quote!(#ty::#ident),
            _ => quote!(#ty::#ident { #(#bound),* }),
        }
    };

    // The reference: the enum's generics after `'a`, each able to live that long.
    let mut ref_generics = input.generics.clone();
    ref_generics.params.insert(0, parse_quote!('a));
    let params: Vec<Ident> = input
        .generics
        .type_params()
        .map(|param| param.ident.clone())
        .collect();
    let ref_where = ref_generics.make_where_clause();
    for param in &params {
        ref_where.predicates.push(parse_quote!(#param: #zp + 'a));
    }
    let (ref_impl_generics, ref_ty_generics, ref_where_clause) = ref_generics.split_for_impl();
    let args = input.generics.params.iter().map(|param| match param {
        GenericParam::Type(param) => param.ident.clone(),
        GenericParam::Const(param) => param.ident.clone(),
        GenericParam::Lifetime(_) => unreachable!("rejected in `expand`"),
    });
    let ref_ty = quote!(#reference<'_, #(#args),*>);

    let ref_variants = variants.iter().map(
        |Variant {
             ident,
             shape,
             fields,
         }| {
            let tys = fields.iter().map(|field| &field.ty);
            let names = fields.iter().map(|field| &field.name);
            match shape {
                Fields::Named(_) => quote!(#ident { #(#names: <#tys as #zp>::Ref<'a>),* }),
                Fields::Unnamed(_) => quote!(#ident(#(<#tys as #zp>::Ref<'a>),*)),
                Fields::Unit => quote!(#ident),
            }
        },
    );

    let variant_sizes = variants
        .iter()
        .map(|variant| size(&variant.fields.iter().collect::<Vec<_>>(), false, context));

    let checks = variants.iter().zip(&tags).map(|(variant, tag)| {
        let tys = variant.fields.iter().map(|field| &field.ty);
        let limits = variant.fields.iter().map(|field| &field.limits);
        quote! {
            ::core::option::Option::Some(#tag) => {
                #(at += <#tys as #zp>::check(#private::from(bytes, at), #limits)?;)*
            }
        }
    });

    let lens = variants.iter().zip(&tags).map(|(variant, tag)| {
        let at = offset(
            &variant.fields.iter().collect::<Vec<_>>(),
            quote!(1),
            context,
        );
        quote!(#tag => #at,)
    });

    let reads = variants.iter().zip(&tags).map(|(variant, tag)| {
        let fields: Vec<&Field> = variant.fields.iter().collect();
        let bound = fields.iter().enumerate().map(|(index, field)| {
            let local = binding(index);
            let ty = &field.ty;
            let at = offset(&fields[..index], quote!(1), context);
            quote!(let #local = <#ty as #zp>::read(#private::from_unchecked(bytes, #at));)
        });
        let value = pattern(&reference, variant);
        quote!(#tag => { #(#bound)* #value })
    });

    let encoded_lens = variants.iter().map(|variant| {
        let tys = variant.fields.iter().map(|field| &field.ty);
        let limits = variant.fields.iter().map(|field| &field.limits);
        let names = (0..variant.fields.len()).map(&binding);
        let value = pattern(name, variant);
        quote! {
            #value => {
                let mut len: usize = 1;
                #(len += <#tys as #zp>::encoded_len(&#zp::input(#names), #limits)?;)*
                len
            }
        }
    });

    let writes = variants.iter().zip(&tags).map(|(variant, tag)| {
        let tys = variant.fields.iter().map(|field| &field.ty);
        let names = (0..variant.fields.len()).map(&binding);
        let value = pattern(name, variant);
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

    let max_lens = variants.iter().map(|variant| {
        let tys = variant.fields.iter().map(|field| &field.ty);
        let limits = variant.fields.iter().map(|field| &field.limits);
        quote! {{
            let mut len: usize = 1;
            #(len = len.checked_add(<#tys as #zp>::max_len(#limits)?)?;)*
            ::core::option::Option::Some(len)
        }}
    });

    let owns = variants.iter().map(|variant| {
        let from = pattern(&reference, variant);
        let owned = variant.fields.iter().enumerate().map(|(index, field)| {
            let (member, ty) = (&field.member, &field.ty);
            let local = binding(index);
            quote!(#member: <#ty as #zp>::own(#local))
        });
        let ident = variant.ident;
        let to = match variant.shape {
            Fields::Unit => quote!(#name::#ident),
            _ => quote!(#name::#ident { #(#owned),* }),
        };
        quote!(#from => #to,)
    });
    let reference_doc = format!("A `{name}` read in place.");

    Ok(quote! {
        #[doc = #reference_doc]
        ///
        /// Match it by value: its hidden variant holds no value, so the match
        /// needs no arm for it.
        #vis enum #reference #ref_generics #ref_where_clause {
            #(#ref_variants,)*
            /// Uses the lifetime and parameters, whatever the fields; it has no value.
            #[doc(hidden)]
            __Borrow(
                ::core::marker::PhantomData<(&'a (), fn() -> #name #ty_generics)>,
                ::core::convert::Infallible,
            ),
        }

        impl #ref_impl_generics #reference #ref_ty_generics #ref_where_clause {
            /// The value, owned.
            #vis fn to_owned(self) -> #name #ty_generics {
                <#name #ty_generics as #zp>::own(self)
            }
        }

        // SAFETY: each method reads the tag, then visits that variant's
        // fields in order, through their own implementations.
        unsafe impl #impl_generics #zp for #name #ty_generics #where_clause {
            type Ref<'a> = #reference #ref_ty_generics where Self: 'a;
            type In<'a> = &'a Self where Self: 'a;
            const SIZE: ::core::option::Option<usize> =
                match #private::same(&[#(#variant_sizes),*]) {
                    ::core::option::Option::Some(size) => ::core::option::Option::Some(1 + size),
                    ::core::option::Option::None => ::core::option::Option::None,
                };

            fn check(
                bytes: &[u8],
                _: &[usize],
            ) -> ::core::result::Result<usize, #krate::Error> {
                // Evaluated here, so its layout checks run for every type.
                let _ = <Self as #zp>::SIZE;
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
                unsafe {
                    #private::len_of::<Self>(bytes, |bytes| match *bytes.get_unchecked(0) {
                        #(#lens)*
                        _ => ::core::hint::unreachable_unchecked(),
                    })
                }
            }

            unsafe fn read(bytes: &[u8]) -> #ref_ty {
                // SAFETY: as in `len`.
                unsafe {
                    match *bytes.get_unchecked(0) {
                        #(#reads)*
                        _ => ::core::hint::unreachable_unchecked(),
                    }
                }
            }

            fn encoded_len(
                value: &&Self,
                _: &[usize],
            ) -> ::core::result::Result<usize, #krate::Error> {
                ::core::result::Result::Ok(match *value {
                    #(#encoded_lens)*
                })
            }

            unsafe fn write(value: &&Self, out: &mut [u8]) -> usize {
                // SAFETY: `out` holds the tag and every field of the variant.
                unsafe {
                    match *value {
                        #(#writes)*
                    }
                }
            }


            #[inline]
            fn max_len(_: &[usize]) -> ::core::option::Option<usize> {
                let mut len: usize = 0;
                #(len = len.max(#max_lens?);)*
                ::core::option::Option::Some(len)
            }

            fn input(&self) -> &Self {
                self
            }

            fn own(value: #ref_ty) -> Self {
                match value {
                    #(#owns)*
                    #reference::__Borrow(_, never) => match never {},
                }
            }
        }
    })
}
