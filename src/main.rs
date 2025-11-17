// this must go first because of macros.
mod config;
mod emit;
mod identifiers;
mod parser_util;
mod util;

use crate::{config::Config, parser_util::JavaClass};
use clap::{Parser, Subcommand};
use std::{
    fs::File,
    io::{self, BufReader, Read},
    path::{Path, PathBuf},
};
use zip::{ZipArchive, read::ZipFile};

/// The core function of this library: Generate Rust code to access Java APIs.
pub fn run(config: impl Into<Config>) -> Result<(), anyhow::Error> {
    let config: Config = config.into();
    println!("output: {}", config.output.display());

    let mut context: emit::Context<'_> = emit::Context::new(&config);
    for file in config.input.iter() {
        gather_file(&mut context, file)?;
    }

    let mut out: Vec<u8> = Vec::with_capacity(4096);
    context.write(&mut out)?;
    util::write_generated(&context, &config.output, &out[..])?;

    // Generate Java proxy files if proxy_output is specified
    if let Some(proxy_output) = &config.proxy_output {
        emit::java_proxy::write_java_proxy_files(&context, proxy_output)?;
    }

    Ok(())
}

fn gather_file(context: &mut emit::Context, path: &Path) -> Result<(), anyhow::Error> {
    let verbose: bool = context.config.logging_verbose;

    context
        .progress
        .lock()
        .unwrap()
        .update(format!("reading {}...", path.display()).as_str());

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
            let class: JavaClass = JavaClass::read(std::fs::read(path)?)?;
            context.add_class(class)?;
        }
        "jar" => {
            let mut jar: ZipArchive<BufReader<File>> =
                ZipArchive::new(BufReader::new(File::open(path)?))?;
            let n: usize = jar.len();

            for i in 0..n {
                let mut file: ZipFile<'_, BufReader<File>> = jar.by_index(i)?;
                if !file.name().ends_with(".class") {
                    continue;
                }

                if verbose {
                    context
                        .progress
                        .lock()
                        .unwrap()
                        .update(format!("  reading {:3}/{}: {}...", i, n, file.name()).as_str());
                }

                let mut buf: Vec<u8> = Vec::new();
                file.read_to_end(&mut buf)?;
                let class: JavaClass = JavaClass::read(buf)?;
                context.add_class(class)?;
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

/// Autogenerate glue code for access Android JVM APIs from Rust
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Generate Java Bindings
    Generate(GenerateCmd),
}

#[derive(Parser, Debug)]
struct GenerateCmd {
    /// Log in more detail
    #[arg(short, long)]
    verbose: bool,

    /// Sets a custom config file
    #[arg(short, long)]
    config: Option<PathBuf>,
}

pub fn main() {
    let cli: Cli = Cli::parse();

    match cli.cmd {
        Cmd::Generate(cmd) => {
            let mut config: Config = if let Some(config_path) = cmd.config {
                config::Config::from_file(&config_path).unwrap()
            } else {
                config::Config::from_current_directory().unwrap()
            };

            if cmd.verbose {
                config.logging_verbose = true;
            }
            run(config).unwrap();
        }
    }
}
