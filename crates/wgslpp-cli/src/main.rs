mod cmd_minify;
mod cmd_pipeline;
mod cmd_preprocess;
mod cmd_reflect;
mod cmd_validate;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "wgslpp", version, about = "WGSL preprocessor, validator, and optimizer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Preprocess a WGSL file (resolve #include, #ifdef, #define)
    Preprocess {
        /// Input WGSL file
        input: PathBuf,
        /// Named package: -P name=path
        #[arg(short = 'P', value_parser = parse_package)]
        packages: Vec<(String, PathBuf)>,
        /// Define: -D name or -D name=value
        #[arg(short = 'D', value_parser = parse_define)]
        defines: Vec<(String, String)>,
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Write source map to file
        #[arg(long = "source-map")]
        source_map: Option<PathBuf>,
    },
    /// Validate a WGSL file
    Validate {
        /// Input WGSL file (already preprocessed, or raw)
        input: PathBuf,
        /// Source map file (to remap error locations)
        #[arg(long = "source-map")]
        source_map: Option<PathBuf>,
        /// Output format
        #[arg(long, default_value = "human")]
        format: DiagnosticFormat,
    },
    /// Extract reflection data from a WGSL file
    Reflect {
        /// Input WGSL file
        input: PathBuf,
        /// Output JSON file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Minify a WGSL file
    Minify {
        /// Input WGSL file
        input: PathBuf,
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// All-in-one build pipeline: preprocess + validate + reflect + minify
    Pipeline {
        /// Input WGSL file
        input: PathBuf,
        /// Named package: -P name=path
        #[arg(short = 'P', value_parser = parse_package)]
        packages: Vec<(String, PathBuf)>,
        /// Define: -D name or -D name=value
        #[arg(short = 'D', value_parser = parse_define)]
        defines: Vec<(String, String)>,
        /// Output WGSL file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Write reflection JSON to file
        #[arg(long)]
        reflect: Option<PathBuf>,
        /// Write source map to file
        #[arg(long = "source-map")]
        source_map: Option<PathBuf>,
        /// Skip validation
        #[arg(long = "no-validate")]
        no_validate: bool,
        /// Skip minification
        #[arg(long = "no-minify")]
        no_minify: bool,
    },
}

#[derive(Clone, Debug)]
enum DiagnosticFormat {
    Human,
    Json,
    Gcc,
}

impl std::str::FromStr for DiagnosticFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "human" => Ok(Self::Human),
            "json" => Ok(Self::Json),
            "gcc" => Ok(Self::Gcc),
            _ => Err(format!("unknown format: {} (expected human, json, gcc)", s)),
        }
    }
}

fn parse_package(s: &str) -> Result<(String, PathBuf), String> {
    let (name, path) = s
        .split_once('=')
        .ok_or_else(|| format!("expected name=path, got: {}", s))?;
    Ok((name.to_string(), PathBuf::from(path)))
}

fn parse_define(s: &str) -> Result<(String, String), String> {
    if let Some((name, value)) = s.split_once('=') {
        Ok((name.to_string(), value.to_string()))
    } else {
        Ok((s.to_string(), String::new()))
    }
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Preprocess {
            input,
            packages,
            defines,
            output,
            source_map,
        } => cmd_preprocess::run(input, packages, defines, output, source_map),
        Commands::Validate {
            input,
            source_map,
            format,
        } => cmd_validate::run(input, source_map, format),
        Commands::Reflect { input, output } => cmd_reflect::run(input, output),
        Commands::Minify { input, output } => cmd_minify::run(input, output),
        Commands::Pipeline {
            input,
            packages,
            defines,
            output,
            reflect,
            source_map,
            no_validate,
            no_minify,
        } => cmd_pipeline::run(
            input,
            packages,
            defines,
            output,
            reflect,
            source_map,
            no_validate,
            no_minify,
        ),
    };

    if let Err(e) = result {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}
