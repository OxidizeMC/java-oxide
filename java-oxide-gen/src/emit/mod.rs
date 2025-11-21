//! Rust generation logic

mod class_proxy;
mod classes;
mod fields;
pub mod java_proxy;
mod known_docs_url;
mod methods;
mod modules;

use self::{classes::Class, modules::Module};
use crate::{config, io_data_err, parser_util};
use proc_macro2::{Literal, Ident, TokenStream};
use quote::{TokenStreamExt, format_ident, quote};
use std::{collections::HashMap, ffi::CString, io, rc::Rc, str::FromStr};

pub struct Context<'a> {
    pub config: &'a config::Config,
    pub module: Module,
    pub all_classes: HashMap<String, Rc<Class>>,
}

impl<'a> Context<'a> {
    pub fn new(config: &'a config::Config) -> Self {
        Self {
            config,
            module: Default::default(),
            all_classes: HashMap::new(),
        }
    }

    pub fn throwable_rust_path(&self, mod_: &str) -> TokenStream {
        self.java_to_rust_path(parser_util::Id("java/lang/Throwable"), mod_)
            .unwrap()
    }

    pub fn java_to_rust_path(
        &self,
        java_class: parser_util::Id,
        curr_mod: &str,
    ) -> Result<TokenStream, anyhow::Error> {
        let jclass_mod: String = Class::mod_for(java_class)?;
        let jclass_name: String = Class::name_for(java_class)?;
        let jclass_path: String = format!("{jclass_mod}::{jclass_name}");

        // // Calculate relative path from B to A.
        // let curr_mod_comps: Vec<&str> = curr_mod.split("::").collect();
        // let jclass_path_comps: Vec<&str> = jclass_path.split("::").collect();
        // let mut jclass_unique_path: &[&str] = &jclass_path_comps[..jclass_path_comps.len() - 1];
        // let mut curr_mod_unique_path: &[&str] = &curr_mod_comps[..];
        // while !jclass_unique_path.is_empty()
        //     && !curr_mod_unique_path.is_empty()
        //     && jclass_unique_path[0] == curr_mod_unique_path[0]
        // {
        //     jclass_unique_path = &jclass_unique_path[1..];
        //     curr_mod_unique_path = &curr_mod_unique_path[1..];
        // }

        // let mut result: TokenStream = TokenStream::new();

        // // for each item left in B, append a `super`
        // for _ in curr_mod_unique_path {
        //     result.extend(quote!(super::));
        // }

        // // for each item in A, append it
        // for ident in jclass_unique_path {
        //     let ident: Ident = format_ident!("{}", ident);
        //     result.extend(quote!(#ident::));
        // }

        // let ident: Ident =
        //     format_ident!("{}", jclass_path_comps[jclass_path_comps.len() - 1]);
        // result.append(ident);


        let mut result: TokenStream = TokenStream::new();

        if jclass_mod == curr_mod {
            result.append(format_ident!("{}", jclass_name));
        } else {
            result.extend(quote!(crate::));

            for ident in jclass_mod.split("::") {
                let ident: Ident = format_ident!("{}", ident);
                result.extend(quote!(#ident::));
            }

            result.append(format_ident!("{}", jclass_name));
        }

        Ok(result)
    }

    pub fn add_class(&mut self, class: parser_util::JavaClass) -> Result<(), anyhow::Error> {
        let class_config: config::ClassConfig<'_> = self.config.resolve_class(class.path().as_str());
        if !class_config.bind {
            return Ok(());
        }

        let java_path: String = class.path().as_str().to_string();
        let class: Rc<Class> = Rc::new(Class::new(class)?);

        self.all_classes.insert(java_path, class.clone());

        let mut rust_mod: &mut Module = &mut self.module;
        for fragment in class.rust.mod_.split("::") {
            rust_mod = rust_mod.modules.entry(fragment.to_owned()).or_default();
        }
        if rust_mod.classes.contains_key(&class.rust.struct_name) {
            return io_data_err!(
                "Unable to add_class(): java class name {:?} was already added",
                &class.rust.struct_name
            )?;
        }
        rust_mod.classes.insert(class.rust.struct_name.clone(), class);

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
