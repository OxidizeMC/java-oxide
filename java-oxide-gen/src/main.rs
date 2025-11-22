// this must go first because of macros.
use crate::{cli::Cli, config::Config, parser_util::JavaClass, prelude::*};
use std::{
    fs::File,
    io::{self, BufReader, Read},
    path::Path,
};
use zip::{ZipArchive, read::ZipFile};

mod cli;
mod config;
mod emit;
mod identifiers;
mod macros;
mod parser_util;
mod setup;
mod util;
pub mod prelude {
    #[allow(unused_imports)]
    pub use log::{debug, error, info, trace, warn};
}

include!(concat!(env!("OUT_DIR"), "/codegen.rs"));

/// The core function of this library: Generate Rust code to access Java APIs.
pub fn run(config: impl Into<Config>) {
    let config: Config = config.into();
    info!("Output: {}", pretty_path!(config.src.output));

    info!("Gathering classes...");
    let mut context: emit::Context<'_> = emit::Context::new(&config);
    for file in config.src.inputs.iter() {
        gather_file(&mut context, file).unwrap();
    }

    let mut out: Vec<u8> = Vec::with_capacity(4096);
    context.write(&mut out).unwrap();
    info!("Writing bindings...");
    match util::write_generated(&config.src.output, &out[..]) {
        Ok(_) => {}
        Err(e) => error!("ERROR WHILE WRITING BINDINGS:\n{}", e),
    };

    // Generate Java proxy files if proxy_output is specified
    // dbg!(&config.proxy.output);
    if let Some(output) = &config.proxy.output {
        match emit::java_proxy::write_java_proxy_files(&context, output) {
            Ok(_) => {}
            Err(e) => error!("ERROR WHILE WRITING PROXIES:\n{}", e),
        };
    }
}

fn gather_file(context: &mut emit::Context, path: &Path) -> Result<(), anyhow::Error> {
    info!("Reading {:?}...", pretty_path!(path));

    let ext: &std::ffi::OsStr = if let Some(ext) = path.extension() {
        ext
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Input files must have an extension",
        ))?;
    };

    match ext.to_string_lossy().to_ascii_lowercase().as_str() {
        "class" => {
            debug!("Reading class directly...");
            let class: JavaClass = JavaClass::read(std::fs::read(path)?)?;
            let class_path: String = class.path().as_str().to_string();
            if !context.add_class(class)? {
                warn!(
                    "Classfile ({:?}) will not be bound because it is not included in the config file!",
                    class_path
                )
            }
        }
        "jar" => {
            let mut jar: ZipArchive<BufReader<File>> =
                ZipArchive::new(BufReader::new(File::open(path)?))?;
            let mut classfiles: Vec<String> = Vec::new();
            for file in jar.file_names() {
                if !file.ends_with(".class") || file.ends_with("package-info.class") {
                    continue;
                }
                classfiles.push(file.to_owned());
            }
            let num_files: usize = classfiles.len();
            let mut num_bound: usize = 0;

            debug!("Reading {} classes from JAR...", num_files);

            #[allow(clippy::unused_enumerate_index)]
            for (_i, file) in classfiles.iter().enumerate() {
                let mut file: ZipFile<'_, BufReader<File>> = jar.by_name(file)?;
                // trace!(
                //     "    Reading {:width$}/{}: {:?}...",
                //     _i + 1,
                //     num_files,
                //     pretty_path!(file.enclosed_name().unwrap()),
                //     width = num_files.checked_ilog10().unwrap_or(0) as usize + 1
                // );
                let mut buf: Vec<u8> = Vec::new();
                file.read_to_end(&mut buf)?;
                let class: JavaClass = JavaClass::read(buf)?;
                if context.add_class(class)? {
                    num_bound += 1;
                }
            }

            if num_bound == 0 {
                warn!(
                    "No classes from the JAR were bound because none of them were included in the config file!"
                );
            } else {
                debug!("{} classes added from JAR", num_bound);
            }
        }
        unknown => {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Input files must have a '.class' or '.jar' extension, not a '.{}' extension",
                    unknown
                ),
            ))?;
        }
    }
    Ok(())
}

pub fn main() {
    let cli: Cli = setup::setup_logger_and_cli();
    info!("Starting...");

    match cli.command {
        cli::Command::Generate(cmd) => {
            let config: Config = if let Some(config_path) = cmd.config {
                config::Config::from_file(&config_path).unwrap()
            } else {
                config::Config::from_current_directory().unwrap()
            };
            run(config);
        }
    }
}
