use super::{
    cstring,
    fields::{RustTypeFlavor, emit_fragment_type, emit_type},
    known_docs_url::KnownDocsUrl,
};
use crate::{
    config::ClassConfig,
    emit::Context,
    identifiers::MethodManglingStyle,
    parser_util::{JavaClass, JavaMethod},
};
use cafebabe::descriptors::{MethodDescriptor, ReturnDescriptor};
use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};

pub struct Method<'a> {
    pub class: &'a JavaClass,
    pub java: JavaMethod<'a>,
    rust_name: Option<String>,
    mangling_style: MethodManglingStyle,
}

impl<'a> Method<'a> {
    pub fn new(class: &'a JavaClass, java: &'a cafebabe::MethodInfo<'a>) -> Self {
        let mut result: Method<'a> = Self {
            class,
            java: JavaMethod::from(java),
            rust_name: None,
            mangling_style: MethodManglingStyle::Java,
        };
        result.set_mangling_style(MethodManglingStyle::Java);
        result
    }

    pub fn rust_name(&self) -> Option<&str> {
        self.rust_name.as_deref()
    }

    pub fn set_mangling_style(&mut self, style: MethodManglingStyle) {
        self.mangling_style = style;
        self.rust_name = self
            .mangling_style
            .mangle(self.java.name(), self.java.descriptor())
            .ok()
    }

    pub fn emit(
        &self,
        context: &Context,
        cc: &ClassConfig,
        mod_: &str,
    ) -> anyhow::Result<TokenStream> {
        let mut emit_reject_reasons: Vec<String> = Vec::new();

        let descriptor: &MethodDescriptor<'_> = self.java.descriptor();

        let method_name: String = if let Some(name) = self.rust_name() {
            name.to_owned()
        } else {
            emit_reject_reasons.push("ERROR: Failed to mangle method name".to_string());
            self.java.name().to_owned()
        };

        if self.java.is_bridge() {
            emit_reject_reasons.push("Bridge method - type erasure".to_string());
        }
        if self.java.is_static_init() {
            emit_reject_reasons
                .push("Static class constructor - never needs to be called by Rust.".to_string());
        }

        // Parameter names may or may not be available as extra debug information.  Example:
        // https://docs.oracle.com/javase/tutorial/reflect/member/methodparameterreflection.html

        let mut params_array: TokenStream = TokenStream::new(); // Contents of let __jni_args = [...];

        // Contents of fn name<'env>(...) {
        let mut params_decl: TokenStream = if self.java.is_constructor() || self.java.is_static() {
            quote!(__jni_env: ::java_oxide::Env<'env>,)
        } else {
            quote!(self: &::java_oxide::Ref<'env, Self>,)
        };

        for (arg_idx, arg) in descriptor.parameters.iter().enumerate() {
            let arg_name: Ident = format_ident!("arg{}", arg_idx);
            let arg_type: TokenStream = emit_type(
                arg,
                context,
                mod_,
                RustTypeFlavor::ImplAsArg,
                &mut emit_reject_reasons,
            )?;

            params_array.extend(quote!(::java_oxide::AsJValue::as_jvalue(&#arg_name),));
            params_decl.extend(quote!(#arg_name: #arg_type,));
        }

        let mut ret_decl: TokenStream =
            if let ReturnDescriptor::Return(desc) = &descriptor.return_type {
                emit_type(
                    desc,
                    context,
                    mod_,
                    RustTypeFlavor::OptionLocal,
                    &mut emit_reject_reasons,
                )?
            } else {
                quote!(())
            };

        let mut ret_method_fragment: &str =
            if let ReturnDescriptor::Return(desc) = &descriptor.return_type {
                emit_fragment_type(desc)
            } else {
                "void"
            };

        if self.java.is_constructor() {
            if descriptor.return_type == ReturnDescriptor::Void {
                ret_method_fragment = "object";
                ret_decl = quote!(::java_oxide::Local<'env, Self>);
            } else {
                emit_reject_reasons.push("ERROR: Constructor should've returned void".to_string());
            }
        }

        if !emit_reject_reasons.is_empty() {
            return Err(anyhow::anyhow!(emit_reject_reasons.join("\n")));
        }

        let mut out: TokenStream = TokenStream::new();

        let attributes: TokenStream = if self.java.deprecated() {
            quote!(#[deprecated])
        } else {
            quote!()
        };

        let docs: String = match KnownDocsUrl::from_method(cc, self) {
            Some(url) => format!("{url}"),
            None => self.java.name().to_string(),
        };

        let throwable: TokenStream = context.throwable_rust_path(mod_);

        let env_let: TokenStream = match !self.java.is_constructor() && !self.java.is_static() {
            true => quote!(let __jni_env = self.env();),
            false => quote!(),
        };
        let require_method: TokenStream = match self.java.is_static() {
            false => quote!(require_method),
            true => quote!(require_static_method),
        };

        let java_name: Literal = cstring(self.java.name());
        let descriptor: Literal = cstring(&self.java.descriptor().to_string());
        let method_name: Ident = format_ident!("{method_name}");

        let call: TokenStream = if self.java.is_constructor() {
            quote!(__jni_env.new_object_a(__jni_class, __jni_method, __jni_args.as_ptr()))
        } else if self.java.is_static() {
            let call: Ident = format_ident!("call_static_{ret_method_fragment}_method_a");
            quote!(    __jni_env.#call(__jni_class, __jni_method, __jni_args.as_ptr()))
        } else {
            let call: Ident = format_ident!("call_{ret_method_fragment}_method_a");
            quote!(    __jni_env.#call(self.as_raw(), __jni_method, __jni_args.as_ptr()))
        };

        out.extend(quote!(
            #[doc = #docs]
            #attributes
            pub fn #method_name<'env>(#params_decl) -> ::std::result::Result<#ret_decl, ::java_oxide::Local<'env, #throwable>> {
                static __METHOD: ::std::sync::OnceLock<::java_oxide::JMethodID> = ::std::sync::OnceLock::new();
                unsafe {
                    let __jni_args = [#params_array];
                    #env_let
                    let __jni_class = Self::__class_global_ref(__jni_env);
                    let __jni_method = __METHOD.get_or_init(||
                        ::java_oxide::JMethodID::from_raw(__jni_env.#require_method(__jni_class, #java_name, #descriptor))
                    ).as_raw();

                    #call
                }
            }
        ));

        Ok(out)
    }
}
