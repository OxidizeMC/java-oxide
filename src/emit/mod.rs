//! Rust generation logic

mod class_proxy;
mod classes;
mod fields;
pub mod java_proxy;
mod known_docs_url;
mod methods;
mod modules;

use self::{classes::Class, modules::Module};
use crate::{config, io_data_err, parser_util, util};
use proc_macro2::{Literal, TokenStream};
use quote::{TokenStreamExt, format_ident, quote};
use std::{
    collections::HashMap, ffi::CString, io, rc::Rc, str::FromStr, sync::Mutex, time::Duration,
};

pub struct Context<'a> {
    pub(crate) config: &'a config::Config,
    pub(crate) module: Module,
    pub(crate) all_classes: HashMap<String, Rc<Class>>,
    pub(crate) progress: Mutex<util::Progress>,
}

impl<'a> Context<'a> {
    pub fn new(config: &'a config::Config) -> Self {
        Self {
            config,
            module: Default::default(),
            all_classes: HashMap::new(),
            progress: Mutex::new(util::Progress::with_duration(Duration::from_millis(
                if config.logging_verbose { 0 } else { 300 },
            ))),
        }
    }

    pub(crate) fn throwable_rust_path(&self, mod_: &str) -> TokenStream {
        self.java_to_rust_path(parser_util::Id("java/lang/Throwable"), mod_)
            .unwrap()
    }

    pub fn java_to_rust_path(
        &self,
        java_class: parser_util::Id,
        mod_: &str,
    ) -> Result<TokenStream, anyhow::Error> {
        let m: String = Class::mod_for(java_class)?;
        let s: String = Class::name_for(java_class)?;
        let fqn: String = format!("{m}::{s}");

        // Calculate relative path from B to A.
        let b: Vec<&str> = mod_.split("::").collect();
        let a: Vec<&str> = fqn.split("::").collect();

        let mut ma: &[&str] = &a[..a.len() - 1];
        let mut mb: &[&str] = &b[..];
        while !ma.is_empty() && !mb.is_empty() && ma[0] == mb[0] {
            ma = &ma[1..];
            mb = &mb[1..];
        }

        let mut res: TokenStream = TokenStream::new();

        // for each item left in b, append a `super`
        for _ in mb {
            res.extend(quote!(super::));
        }

        // for each item in a, append it
        for ident in ma {
            let ident: proc_macro2::Ident = format_ident!("{}", ident);
            res.extend(quote!(#ident::));
        }

        let ident: proc_macro2::Ident = format_ident!("{}", a[a.len() - 1]);
        res.append(ident);

        Ok(res)
    }

    pub fn add_class(&mut self, class: parser_util::JavaClass) -> Result<(), anyhow::Error> {
        let cc: config::ClassConfig<'_> = self.config.resolve_class(class.path().as_str());
        if !cc.include {
            return Ok(());
        }

        let java_path: String = class.path().as_str().to_string();
        let s: Rc<Class> = Rc::new(Class::new(class)?);

        self.all_classes.insert(java_path, s.clone());

        let mut rust_mod: &mut Module = &mut self.module;
        for fragment in s.rust.mod_.split("::") {
            rust_mod = rust_mod.modules.entry(fragment.to_owned()).or_default();
        }
        if rust_mod.classes.contains_key(&s.rust.struct_name) {
            return io_data_err!(
                "Unable to add_class(): java class name {:?} was already added",
                &s.rust.struct_name
            )?;
        }
        rust_mod.classes.insert(s.rust.struct_name.clone(), s);

        Ok(())
    }

    pub fn write(&self, out: &mut impl io::Write) -> anyhow::Result<()> {
        write!(out, "{}\n\n", include_str!("preamble.rs"))?;
        self.module.write(self, out)
    }
}

fn cstring(s: &str) -> Literal {
    Literal::c_string(&CString::from_str(s).unwrap())
}
