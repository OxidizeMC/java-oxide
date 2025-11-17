# ☕️🦀 `java-oxide`

Easily generate type-safe bindings for calling Java APIs from Rust.

## Features

- Generates fully safe Rust bindings to call Java APIs.
- Supports both static and nonstatic methods and fields.
- Constant fields (`static final`) are converted to Rust constants.
- Allows implementing Java interfaces or subclassing Java classes using generated Java proxies. Useful for callback/listener APIs. It generates:
  - a Rust trait matching the Java interface/class for you to implement
  - a proxy Java class where all methods call into Rust
  - Rust glue to receive and forward calls to the Rust trait
- Flexible configuration based on glob rules matching Java classes. All matching rules are merged.
- Actual documentation (when compared to `java-spaghetti-gen`)

## Differences vs `java-spaghetti` (& `jni-bindgen`)

This project started out as a fork of [`java-spaghetti`](https://github.com/Dirbaio/java-spaghetti), which is originally a fork [`jni-bindgen`](https://github.com/MaulingMonkey/jni-bindgen).

`java-spaghetti` took the route of generating "mini-bindings", that are tailored specifically to your project, and can be embeded within your crate. This design, however, is not suited for large-scale bindings that are intended to be used as/in libraries. `java-oxide` partially goes back to the design of `jni-bindgen`, it generates crates with bindings for a whole java API (or multiple). `java-oxide` still uses `java-spaghetti`'s JNI wrapper library because it is very useful, this is just a fork of the bindgen portion.

The list of differences from `java-spaghetti` are:

- Configuration using TOML instead of YAML, to better fit in with the rest of the rust ecosystem.
- Generated code uses absolute paths (`crate::...`) instead of relative paths (`super::...`), because relative path chains in a multifile/crate layout get huge FAST.
- Code is generated as multiple files following the package layout of the source JARs.
- Generated code does use macros, cause it's easier to read that way (at least to me).
- EVEN MORE modernized rust and updated dependencies. `java-spaghetti` is quite stale, and also slightly broken on newer rust versions.

A list of differences from `jni-bindgen` are listed in [`java-spaghetti`'s README](https://github.com/Dirbaio/java-spaghetti).

## TODO

- [ ] Switch back to a `.toml` configuration format
- [ ] Fix formatting issues
  - [ ] Use absolute paths (`crate::...`)
  - [ ] "Correctly" format generated code
- [ ] Use `ristretto_classfile` instead of `cafebabe`
- [ ] Fix indeterminism in what JARs are read and when
- [ ] Actually do documentation
- [ ] Stub generation?

## License

Licensed under the MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
