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
- The ability to use pre-generated binding sources instead of having to rebind whole ABIs
- Generate stubbed JARs that you can include in your source code without issues (Always read the applicable licenses first, though)
- Actual documentation on how to generate bindings (when compared to `java-spaghetti-gen`)

## Differences vs `java-spaghetti` (& `jni-bindgen`)

This project started out as a fork of [`java-spaghetti`](https://github.com/Dirbaio/java-spaghetti), which is originally a fork [`jni-bindgen`](https://github.com/MaulingMonkey/jni-bindgen).

`java-spaghetti` took the route of generating "mini-bindings", that are tailored specifically to your project, and can be embeded within your crate. This design, however, is not suited for large-scale bindings that are intended to be used as/in libraries. `java-oxide` partially goes back to the design of `jni-bindgen`, it generates crates with bindings for a whole java API (or multiple).

A list of differences from `java-spaghetti` are:

- Configuration using TOML instead of YAML, to better fit in with the rest of the rust ecosystem.
- Generated code uses absolute paths (`crate::...`) instead of relative paths (`super::...`), because relative path chains can be confusing to read.
- (NOT IMPLEMENTED YET) Code is generated as multiple files following the package layout of the source JARs.
- (NOT IMPLEMENTED YET) Generated code does use some macros, cause it's easier to read that way (at least to me).
- EVEN MORE modernized rust and updated dependencies. `java-spaghetti` is quite stale, and also slightly broken on newer rust versions.
- (NOT IMPLEMENTED YET) The ability to use pre-generated binding sources instead of having to rebind whole ABIs
- Better logging and error reporting

A list of differences from `jni-bindgen` are listed in [`java-spaghetti`'s README](https://github.com/Dirbaio/java-spaghetti).

## TODO

- [X] Switch back to a `.toml` configuration format
- [X] Allow Glob Paths as inputs
- [ ] Add external pre-generated binding sources
- [ ] Fix code gen issues
  - [X] Use absolute paths (`crate::...`)
  - [ ] Use SOME macros to improve readability
  - [ ] "Correctly" format generated code
  - [ ] Report to user what missing classes are causing incomplete code generations
- [ ] Use `ristretto_classfile` instead of `cafebabe`
- [ ] Implement stub generation
- [ ] Actually do documentation

## License

Licensed under the MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
