use crate::{prelude::*, pretty_path};
use serde::Deserialize;
use soft_canonicalize::soft_canonicalize;
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};

fn default_proxy_package() -> String {
    "java_oxide.proxy".to_string()
}
fn default_slash() -> String {
    "/".to_string()
}
fn default_period() -> String {
    ".".to_string()
}
fn default_comma() -> String {
    ",".to_string()
}

/// Configuration for what classes to bind/proxy
#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct IncludeConfig {
    /// What java class(es) to match against. This takes the form of a glob pattern matching JNI paths.
    ///
    /// Glob patterns are case-sensitive and require literal path separators (/ cannot be matched by *).
    /// Use ** to match across directory boundaries.
    ///
    /// | To Match:                 | Use a glob pattern:                   |
    /// | ------------------------- | ------------------------------------- |
    /// | *                         | "*"
    /// | java.lang.*               | "java/lang/**"
    /// | name.spaces.OuterClass.*  | "name/spaces/OuterClass$*"
    /// | Specific class            | "com/example/MyClass"
    /// | Multiple specific classes | ["com/example/Class1", "com/example/Class2"]
    #[serde(rename = "match")]
    pub matches: Vec<String>,

    /// Whether to generate Java bindings
    #[serde(default)]
    pub bind: Option<bool>,

    #[serde(default)]
    pub bind_private_classes: Option<bool>,
    #[serde(default)]
    pub bind_private_methods: Option<bool>,
    #[serde(default)]
    pub bind_private_fields: Option<bool>,

    /// Whether to generate Java proxies. Setting to 'proxy = true' will force 'bind = true'
    #[serde(default)]
    pub proxy: Option<bool>,
}
impl IncludeConfig {
    pub fn check(&self) -> Result<(), Vec<&'static str>> {
        let mut errors: Vec<&'static str> = Vec::new();
        if self.matches.is_empty() {
            errors.push("At least one match pattern must be specified in 'include.match'");
        }
        if self.matches.iter().any(|x: &String| x.is_empty()) {
            errors.push("Zero length strings are not allowed in 'include.match'");
        }
        if self.bind.is_none() && self.proxy.is_none() {
            errors.push("Either 'include.bind' or 'include.proxy' must be set to true");
        }
        if self.bind.is_none()
            && (self.bind_private_classes.is_some()
                || self.bind_private_fields.is_some()
                || self.bind_private_methods.is_some())
        {
            errors.push(
                "'include.bind' must also be set to true if any 'include.bind-private-*' values are set to true",
            );
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(())
    }

    pub fn matches_class(&self, class: &str) -> bool {
        let options: glob::MatchOptions = glob::MatchOptions {
            case_sensitive: true,
            require_literal_separator: true,
            require_literal_leading_dot: false,
        };

        self.matches.iter().any(|p: &String| -> bool {
            let pattern: glob::Pattern =
                glob::Pattern::new(p).unwrap_or_else(|e: glob::PatternError| -> glob::Pattern {
                    panic!("Invalid glob pattern '{p}': {e}")
                });
            pattern.matches_with(class, options)
        })
    }
}

/// Configuration for Documentation URL patterns
#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct DocConfig {
    /// What java class(es) to match against.  This takes the form of a glob pattern matching JNI paths.
    ///
    /// Glob patterns are case-sensitive and require literal path separators (/ cannot be matched by *).
    /// Use ** to match across directory boundaries.
    ///
    /// | To Match:                 | Use a glob pattern:                   |
    /// | ------------------------- | ------------------------------------- |
    /// | *                         | "*"
    /// | java.lang.*               | "java/lang/**"
    /// | name.spaces.OuterClass.*  | "name/spaces/OuterClass$*"
    /// | Specific class            | "com/example/MyClass"
    /// | Multiple specific classes | ["com/example/Class1", "com/example/Class2"]
    #[serde(rename = "match")]
    pub matches: Vec<String>,

    /// The URL to use for documenting a given class.  `{CLASS}` will be replaced with everything *after* the JNI prefix.
    ///
    /// | Given:                | Use this if you want android documentation:   |
    /// | --------------------- | --------------------------------------------- |
    /// | jni_prefix = "java/"  | class_url_pattern = "https://developer.android.com/reference/java/{CLASS}.html"
    /// | jni_prefix = ""       | class_url_pattern = "https://developer.android.com/reference/{CLASS}.html"
    #[serde(default)]
    pub class_url: Option<String>,

    /// The URL to use for documenting a given class method.
    ///
    /// * `{CLASS}` will be replaced with everything *after* the JNI prefix.
    /// * `{METHOD}` will be replaced with the method name.
    /// * `{ARGUMENTS}` will be replaced with the method arguments.
    ///
    /// | Given:                | Use this if you want android documentation:   |
    /// | --------------------- | --------------------------------------------- |
    /// | jni_prefix = "java/"  | method_url_pattern = "https://developer.android.com/reference/java/{CLASS}.html#{METHOD}({ARGUMENTS})"
    /// | jni_prefix = ""       | method_url_pattern = "https://developer.android.com/reference/{CLASS}.html#{METHOD}({ARGUMENTS})"
    #[serde(default)]
    pub method_url: Option<String>,

    /// The URL to use for documenting a given class field.
    ///
    /// * `{CLASS}` will be replaced with everything *after* the JNI prefix.
    /// * `{FIELD}` will be replaced with the field name.
    ///
    /// | Given:                | Use this if you want android documentation:   |
    /// | --------------------- | --------------------------------------------- |
    /// | jni_prefix = "java/"  | field_url_pattern = "https://developer.android.com/reference/java/{CLASS}.html#{FIELD}"
    /// | jni_prefix = ""       | field_url_pattern = "https://developer.android.com/reference/{CLASS}.html#{FIELD}"
    #[serde(default)]
    pub field_url: Option<String>,

    /// The URL to use for documenting a given class constructor.
    ///
    /// * `{CLASS}` will be replaced with everything *after* the JNI prefix.
    /// * `{CLASS.OUTER}` will be replaced with just the class name, including the outer class(es)
    /// * `{CLASS.INNER}` will be replaced with just the class name, excluding the outer class(es)
    /// * `{METHOD}` aliases `{CLASS.INNER}`
    /// * `{ARGUMENTS}` will be replaced with the method arguments.
    ///
    /// Defaults to method_url_pattern
    ///
    /// | Given:                | Use this if you want android documentation:   |
    /// | --------------------- | --------------------------------------------- |
    /// | jni_prefix = "java/"  | constructor_url_pattern = "https://developer.android.com/reference/java/{CLASS}.html#{CLASS.INNER}({ARGUMENTS})"
    /// | jni_prefix = ""       | constructor_url_pattern = "https://developer.android.com/reference/{CLASS}.html#{CLASS.INNER}({ARGUMENTS})"
    #[serde(default)]
    pub constructor_url: Option<String>,

    /// Configuration for what separators to use when generating Documentation URLs
    #[serde(default)]
    pub sep: DocSepConfig,
}
impl DocConfig {
    pub fn check(&self) -> Result<(), Vec<&'static str>> {
        let mut errors: Vec<&'static str> = Vec::new();
        if self.matches.is_empty() {
            errors.push("At least one match pattern must be specified in 'doc.match'");
        }
        if self.matches.iter().any(|x: &String| x.is_empty()) {
            errors.push("Zero length strings are not allowed in 'doc.match'");
        }
        if self.class_url.is_none()
            && self.constructor_url.is_none()
            && self.field_url.is_none()
            && self.method_url.is_none()
        {
            errors.push("At least one documentation url pattern must be specified in 'doc'");
        }
        if let Some(url) = &self.class_url
            && url.is_empty()
        {
            errors.push("'doc.class-url' cannot be an empty string");
        }
        if let Some(url) = &self.method_url
            && url.is_empty()
        {
            errors.push("'doc.method-url' cannot be an empty string");
        }
        if let Some(url) = &self.field_url
            && url.is_empty()
        {
            errors.push("'doc.field-url' cannot be an empty string");
        }
        if let Some(url) = &self.constructor_url
            && url.is_empty()
        {
            errors.push("'doc.constructor-url' cannot be an empty string");
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(())
    }

    pub fn matches_class(&self, class: &str) -> bool {
        let options: glob::MatchOptions = glob::MatchOptions {
            case_sensitive: true,
            require_literal_separator: true,
            require_literal_leading_dot: false,
        };

        self.matches.iter().any(|p: &String| -> bool {
            let pattern: glob::Pattern =
                glob::Pattern::new(p).unwrap_or_else(|e: glob::PatternError| -> glob::Pattern {
                    panic!("Invalid glob pattern '{p}': {e}")
                });
            pattern.matches_with(class, options)
        })
    }
}

/// Configuration for what separators to use when generating Documentation URLs
#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct DocSepConfig {
    /// What to use in the {CLASS} portion of URLs to separate namespaces.  Defaults to "/".
    #[serde(default = "default_slash")]
    pub class_namespace: String,

    /// What to use in the {CLASS} portion of URLs to separate inner classes from outer classes.  Defaults to ".".
    #[serde(default = "default_period")]
    pub class_inner_class: String,

    /// What to use in the {ARGUMENTS} portion of URLs to separate inner classes from outer classes.  Defaults to ",".
    #[serde(default = "default_period")]
    pub argument: String,

    /// What to use in the {ARGUMENTS} portion of URLs to separate namespaces.  Defaults to ".".
    #[serde(default = "default_period")]
    pub argument_namespace: String,

    /// What to use in the {ARGUMENTS} portion of URLs to separate inner classes from outer classes.  Defaults to ".".
    #[serde(default = "default_comma")]
    pub argument_inner_class: String,
}
impl Default for DocSepConfig {
    fn default() -> Self {
        Self {
            class_namespace: default_slash(),
            class_inner_class: default_period(),
            argument: default_period(),
            argument_namespace: default_period(),
            argument_inner_class: default_comma(),
        }
    }
}

/// Configuration for Java proxy generation
#[derive(Deserialize, Debug)]
pub struct ProxyConfig {
    /// The Java package location for generated proxies
    #[serde(default = "default_proxy_package")]
    pub package: String,
    /// Where to place the generated proxies
    #[serde(default)]
    pub output: Option<PathBuf>,
}
impl ProxyConfig {
    pub fn check(&self) -> Result<(), Vec<&'static str>> {
        let mut errors: Vec<&'static str> = Vec::new();
        if self.package.is_empty() {
            errors.push("'proxy.package' cannot be an empty string");
        }
        if let Some(output) = &self.output
            && output.as_os_str().is_empty()
        {
            errors.push("'proxy.output' cannot be an empty string");
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(())
    }
}
impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            package: default_proxy_package(),
            output: None,
        }
    }
}

/// Configuration for binding generation sources
#[derive(Deserialize, Debug)]
pub struct SourceConfig {
    /// A list of path(s) to JAR input(s)
    pub inputs: Vec<PathBuf>,
    /// Where to place generated bindings
    pub output: PathBuf,
}
impl SourceConfig {
    pub fn check(&self) -> Result<(), Vec<&'static str>> {
        let mut errors: Vec<&'static str> = Vec::new();
        if self.inputs.is_empty() {
            errors.push("At least one input JAR must be specified in 'source.inputs'");
        }
        if self.inputs.iter().any(|x| x.as_os_str().is_empty()) {
            errors.push("Empty strings are not allowed in 'source.inputs'");
        }
        if self.output.as_os_str().is_empty() {
            errors.push("'source.output' cannot be an empty string");
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(())
    }
}

#[derive(Deserialize, Debug)]
pub struct Config {
    /// Configuration for binding generation sources
    #[serde(rename = "sources")]
    pub src: SourceConfig,

    /// Optional configuration for Java proxy generation
    #[serde(default)]
    pub proxy: ProxyConfig,

    /// Optional list of configurations for Documentation URL patterns
    #[serde(default)]
    #[serde(rename = "doc")]
    pub docs: Option<Vec<DocConfig>>,

    /// List of configurations for what classes to bind/proxy
    #[serde(rename = "include")]
    pub rules: Vec<IncludeConfig>,
}

impl Config {
    /// Read config from I/O, under the assumption that it's in the "java-oxide.toml" file format.
    /// `dir` is the directory that contained the `java-oxide.toml` file, against which paths should be resolved.
    pub fn read(file: &mut impl io::Read, dir: &Path) -> io::Result<Self> {
        let mut buffer: String = String::new();
        file.read_to_string(&mut buffer)?;
        let mut config: Config = match toml::from_str(&buffer[..]) {
            Ok(c) => c,
            Err(e) => panic!("Failed to parse config file:\n{}", e),
        };
        config.check();

        config.src.output = resolve_file(&config.src.output, dir)?;
        if let Some(output) = &mut config.proxy.output {
            *output = resolve_file(output, dir)?;
        }
        for f in &mut config.src.inputs {
            *f = resolve_file(f, dir)?
        }

        config.proxy.package = config.proxy.package.replace(".", "/");
        if let Some(docs) = &mut config.docs {
            for doc in docs {
                for class in &mut doc.matches {
                    *class = class.replace(".", "/");
                }
            }
        }
        for rule in &mut config.rules {
            for class in &mut rule.matches {
                *class = class.replace(".", "/");
            }
        }

        // dbg!(&config);
        Ok(config)
    }

    // Search the current directory - or failing that, it's ancestors - until we find "java-oxide.toml" or reach the
    /// root of the filesystem and cannot continue.
    #[allow(dead_code)]
    pub fn from_current_directory() -> io::Result<Self> {
        let current_dir: PathBuf = soft_canonicalize(std::env::current_dir()?)?;
        let mut path: PathBuf = current_dir.to_owned();

        loop {
            path.push("java-oxide.toml");
            if path.exists() {
                info!("Found config file: {:?}", pretty_path!(&path));
                return Self::read(&mut File::open(&path)?, path.parent().unwrap());
            }
            if !path.pop() || !path.pop() {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!(
                        "Failed to find 'java-oxide.toml' in {:?} or any of it's parent directories.",
                        pretty_path!(current_dir)
                    ),
                ))?;
            }
        }
    }

    /// Search the specified directory - or failing that, it's ancestors - until we find "java-oxide.toml" or reach the
    /// root of the filesystem and cannot continue.
    pub fn from_file(path: &Path) -> io::Result<Self> {
        let current_dir: PathBuf = std::env::current_dir()?;
        let path: PathBuf = soft_canonicalize(path)?;
        let mut file: File = File::open(&path)?;
        let config_dir: &Path = path.parent().unwrap_or(current_dir.as_path());
        Self::read(&mut file, config_dir)
    }

    /// Panics if any field in Config is not valid
    fn check(&self) {
        let mut errors: Vec<&'static str> = Vec::new();

        if let Err(e) = self.src.check() {
            errors.extend(e);
        }
        if let Err(e) = self.proxy.check() {
            errors.extend(e);
        }
        if let Some(docs) = &self.docs {
            for doc in docs {
                if let Err(e) = doc.check() {
                    errors.extend(e);
                }
            }
        }
        for include in &self.rules {
            if let Err(e) = include.check() {
                errors.extend(e);
            }
        }

        if !errors.is_empty() {
            panic!(
                "Incorrect config format! Errors:\n    {}",
                errors.join("\n    ")
            )
        }
    }

    pub fn resolve_class(&self, class: &str) -> ClassConfig<'_> {
        // false > true > None
        let mut temp_config: TempClassConfig = TempClassConfig {
            bind: None,
            bind_private_classes: None,
            bind_private_methods: None,
            bind_private_fields: None,
            proxy: None,
        };

        for rule in &self.rules {
            if rule.matches_class(class) {
                if temp_config.bind.is_none() {
                    temp_config.bind = rule.bind;
                } else if temp_config.bind == Some(true) && rule.bind.is_some() {
                    temp_config.bind = rule.bind;
                }

                if temp_config.bind_private_classes.is_none() {
                    temp_config.bind_private_classes = rule.bind_private_classes;
                } else if temp_config.bind_private_classes == Some(true)
                    && rule.bind_private_classes.is_some()
                {
                    temp_config.bind_private_classes = rule.bind_private_classes;
                }

                if temp_config.bind_private_methods.is_none() {
                    temp_config.bind_private_methods = rule.bind_private_methods;
                } else if temp_config.bind_private_methods == Some(true)
                    && rule.bind_private_methods.is_some()
                {
                    temp_config.bind_private_methods = rule.bind_private_methods;
                }

                if temp_config.bind_private_fields.is_none() {
                    temp_config.bind_private_fields = rule.bind_private_fields;
                } else if temp_config.bind_private_fields == Some(true)
                    && rule.bind_private_fields.is_some()
                {
                    temp_config.bind_private_fields = rule.bind_private_fields;
                }

                if temp_config.proxy.is_none() {
                    temp_config.proxy = rule.proxy;
                } else if temp_config.proxy == Some(true) && rule.proxy.is_some() {
                    temp_config.proxy = rule.proxy;
                }
            }
        }

        if temp_config.bind == Some(false) {
            temp_config.bind_private_classes = Some(false);
            temp_config.bind_private_methods = Some(false);
            temp_config.bind_private_fields = Some(false);
        }

        if temp_config.proxy == Some(true) {
            temp_config.bind = Some(true);
        }

        let mut res: ClassConfig<'_> = ClassConfig {
            bind: temp_config.bind.unwrap_or_default(),
            bind_private_classes: temp_config.bind_private_classes.unwrap_or_default(),
            bind_private_methods: temp_config.bind_private_methods.unwrap_or_default(),
            bind_private_fields: temp_config.bind_private_fields.unwrap_or_default(),
            proxy: temp_config.proxy.unwrap_or_default(),
            doc_pattern: None,
        };

        if let Some(docs) = &self.docs {
            for pattern in docs {
                if pattern.matches_class(class) {
                    res.doc_pattern = Some(pattern);
                }
            }
        }

        res
    }
}

#[derive(Debug, Clone)]
pub struct ClassConfig<'a> {
    pub bind: bool,
    pub bind_private_classes: bool,
    pub bind_private_methods: bool,
    pub bind_private_fields: bool,
    pub proxy: bool,
    pub doc_pattern: Option<&'a DocConfig>,
}

#[derive(Debug)]
pub struct TempClassConfig {
    pub bind: Option<bool>,
    pub bind_private_classes: Option<bool>,
    pub bind_private_methods: Option<bool>,
    pub bind_private_fields: Option<bool>,
    pub proxy: Option<bool>,
}

fn resolve_file(path: &Path, dir: &Path) -> io::Result<PathBuf> {
    let path: PathBuf = path.into();
    if path.is_relative() {
        let p: PathBuf = dir.join(&path);
        trace!("Resolving path (relative)  {:?}  -->  {:?}", &path, &p);
        soft_canonicalize(p)
    } else {
        trace!("Reasolving path (absolute)  {:?}", &path);
        soft_canonicalize(path)
    }
}
