use super::{cstring, fields::Field, known_docs_url::KnownDocsUrl, methods::Method};
use crate::{
    config::ClassConfig,
    emit::Context,
    identifiers::{FieldMangling, MethodManglingStyle, rust_ident},
    parser_util::{Id, IdPart, JavaClass},
    prelude::*,
};
use cafebabe::{FieldInfo, MethodInfo, descriptors::ClassName};
use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use std::{
    collections::{HashMap, HashSet},
    fmt::Write,
};

#[derive(Debug, Default)]
pub struct StructPaths {
    pub mod_: String,
    pub struct_name: String,
}

impl StructPaths {
    pub fn new(class: Id) -> Result<Self, anyhow::Error> {
        Ok(Self {
            mod_: Class::mod_for(class)?,
            struct_name: Class::name_for(class)?,
        })
    }
}

#[derive(Debug)]
pub struct Class {
    pub rust: StructPaths,
    pub java: JavaClass,
}

impl Class {
    pub fn mod_for(class: Id) -> Result<String, anyhow::Error> {
        let mut buf: String = String::new();
        for component in class {
            match component {
                IdPart::Namespace(id) => {
                    if !buf.is_empty() {
                        buf.push_str("::");
                    }
                    buf.push_str(&rust_ident(id)?);
                }
                IdPart::ContainingClass(_) => {}
                IdPart::LeafClass(_) => {}
            }
        }
        Ok(buf)
    }

    pub fn name_for(class: Id) -> Result<String, anyhow::Error> {
        let mut buf: String = String::new();
        for component in class.iter() {
            match component {
                IdPart::Namespace(_) => {}
                IdPart::ContainingClass(id) => write!(&mut buf, "{}_", rust_ident(id)?)?,
                IdPart::LeafClass(id) => write!(&mut buf, "{}", rust_ident(id)?)?,
            }
        }
        Ok(buf)
    }

    pub fn new(java: JavaClass) -> Result<Self, anyhow::Error> {
        let rust: StructPaths = StructPaths::new(java.path())?;

        Ok(Self { rust, java })
    }

    pub fn write(&self, context: &Context) -> anyhow::Result<TokenStream> {
        let cc: ClassConfig<'_> = context.config.resolve_class(self.java.path().as_str());

        // Ignored access_flags: SUPER, SYNTHETIC, ANNOTATION, ABSTRACT

        let keyword: &str = if self.java.is_interface() {
            "interface"
        } else if self.java.is_enum() {
            "enum"
        } else if self.java.is_static() {
            "static class"
        } else if self.java.is_final() {
            "final class"
        } else {
            "class"
        };

        let visibility: TokenStream = if self.java.is_public() || cc.bind_private_classes {
            quote!(pub)
        } else {
            quote!()
        };
        let attributes: TokenStream = match self.java.deprecated() {
            true => quote!(#[deprecated] ),
            false => quote!(),
        };

        let docs: String = match KnownDocsUrl::from_class(&cc, self.java.path()) {
            Some(url) => format!("{keyword} {url}"),
            None => format!("{keyword} {}", self.java.path().as_str()),
        };

        let rust_name: Ident = format_ident!("{}", &self.rust.struct_name);

        let referencetype_impl: TokenStream = match self.java.is_static() {
            true => quote!(),
            false => quote!(unsafe impl ::java_oxide::ReferenceType for #rust_name {}),
        };

        let mut out: TokenStream = TokenStream::new();

        let java_path: Literal = cstring(self.java.path().as_str());

        out.extend(quote!(
            #[doc = #docs]
            #attributes
            #visibility enum #rust_name {}

            #referencetype_impl

            unsafe impl ::java_oxide::JniType for #rust_name {
                fn static_with_jni_type<R>(callback: impl FnOnce(&::std::ffi::CStr) -> R) -> R {
                    callback(#java_path)
                }
            }
        ));

        // recursively visit all superclasses and superinterfaces.
        let mut queue: Vec<Id<'_>> = Vec::new();
        let mut visited: HashSet<Id<'_>> = HashSet::new();
        queue.push(self.java.path());
        visited.insert(self.java.path());
        while let Some(path) = queue.pop() {
            let class: &std::rc::Rc<Class> = context.all_classes.get(path.as_str()).unwrap();
            for path2 in self
                .java
                .interfaces()
                .map(|i: &ClassName<'_>| Id(i))
                .chain(class.java.super_path())
            {
                if context.all_classes.contains_key(path2.as_str()) && !visited.contains(&path2) {
                    let rust_path: TokenStream =
                        context.java_to_rust_path(path2, &self.rust.mod_).unwrap();
                    out.extend(quote!(
                        unsafe impl ::java_oxide::AssignableTo<#rust_path> for #rust_name {}
                    ));
                    queue.push(path2);
                    visited.insert(path2);
                }
            }
        }

        let mut contents: TokenStream = TokenStream::new();

        let object: TokenStream = context
            .java_to_rust_path(Id("java/lang/Object"), &self.rust.mod_)
            .unwrap();

        let class: Literal = cstring(self.java.path().as_str());

        contents.extend(quote!(
            fn __class_global_ref(__jni_env: ::java_oxide::Env) -> ::java_oxide::sys::jobject {
                static __CLASS: ::std::sync::OnceLock<::java_oxide::Global<#object>> = ::std::sync::OnceLock::new();
                __CLASS
                    .get_or_init(|| unsafe {
                        ::java_oxide::Local::from_raw(__jni_env, __jni_env.require_class(#class)).as_global()
                    })
                    .as_raw()
            }
        ));

        let mut methods: Vec<Method> = self
            .java
            .methods()
            .map(|m: &MethodInfo<'_>| Method::new(&self.java, m))
            .filter(|m: &Method<'_>| {
                (m.java.is_public() || cc.bind_private_methods) && !m.java.is_bridge()
            })
            .collect();
        let mut fields: Vec<Field> = self
            .java
            .fields()
            .map(|f: &FieldInfo<'_>| Field::new(&self.java, f))
            .filter(|f: &Field<'_>| f.java.is_public() || cc.bind_private_fields)
            .collect();

        self.resolve_collisions(&mut methods, &fields)?;

        for method in &mut methods {
            match method.emit(context, &cc, &self.rust.mod_) {
                Ok(res) => contents.extend(res),
                Err(e) => trace!(
                    "METHOD REJECTED - \"{}.{}()\":\n{}",
                    self.java.path().as_str(),
                    method.java.name(),
                    e
                ),
            };
        }

        for field in &mut fields {
            match field.emit(context, &cc, &self.rust.mod_) {
                Ok(res) => contents.extend(res),
                Err(e) => trace!(
                    "FIELD REJECTED - \"{}.{}\":\n{}",
                    self.java.path().as_str(),
                    field.java.name(),
                    e
                ),
            };
        }

        out.extend(quote!(impl #rust_name { #contents }));

        if cc.proxy {
            out.extend(self.write_proxy(context, &methods)?);
        }

        Ok(out)
    }

    /// Fills the name_counts map with all field and method names
    fn fill_name_counts(&self, methods: &[Method], fields: &[Field]) -> HashMap<String, usize> {
        let mut name_counts: HashMap<String, usize> = HashMap::new();

        // Fill name_counts with all names from fields
        for field in fields {
            match field.rust_names.as_ref() {
                Ok(FieldMangling::ConstValue(name, _)) => {
                    *name_counts.entry(name.clone()).or_insert(0) += 1;
                }
                Ok(FieldMangling::GetSet(get, set)) => {
                    *name_counts.entry(get.clone()).or_insert(0) += 1;
                    *name_counts.entry(set.clone()).or_insert(0) += 1;
                }
                Err(_) => {}
            }
        }

        // Fill name_counts with all names from methods
        for method in methods.iter() {
            if let Some(name) = method.rust_name() {
                *name_counts.entry(name.to_owned()).or_insert(0) += 1;
            }
        }

        name_counts
    }

    /// Resolves method name collisions using a hardcoded fallback strategy:
    /// Java -> JavaShortSignature -> JavaLongSignature
    /// Only colliding methods are upgraded to the next mangling level.
    fn resolve_collisions(&self, methods: &mut [Method], fields: &[Field]) -> anyhow::Result<()> {
        // Start with all methods using Java style
        for method in methods.iter_mut() {
            method.set_mangling_style(MethodManglingStyle::Java);
        }

        for style in [
            MethodManglingStyle::JavaShortSignature,
            MethodManglingStyle::JavaLongSignature,
        ] {
            let name_counts: std::collections::HashMap<String, usize> =
                self.fill_name_counts(methods, fields);

            let has_collisions: bool = name_counts.values().any(|&count| count >= 2);
            if !has_collisions {
                return Ok(()); // All names are unique, we're done
            }

            // Upgrade methods that have collisions to the next mangling style
            for method in methods.iter_mut() {
                if let Some(name) = method.rust_name()
                    && name_counts.get(name).unwrap_or(&0) >= &2
                {
                    method.set_mangling_style(style);
                }
            }
        }

        let name_counts: HashMap<String, usize> = self.fill_name_counts(methods, fields);
        let has_collisions: bool = name_counts.values().any(|&count| count >= 2);
        if !has_collisions {
            return Ok(()); // All names are unique, we're done
        }

        // we still have collisions, return an error
        let conflicting_names: Vec<String> = name_counts
            .into_iter()
            .filter(|(_, count)| *count >= 2)
            .map(|(name, _)| name)
            .collect();

        Err(anyhow::anyhow!(
            "Unable to resolve method name collisions in class {}: {}",
            self.java.path().as_str(),
            conflicting_names.join(", ")
        ))
    }
}
