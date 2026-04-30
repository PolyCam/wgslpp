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
        /// Input WGSL file (omit when using --stdin)
        input: Option<PathBuf>,
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
        /// Config file (wgslpp.json) for package resolution
        #[arg(long)]
        config: Option<PathBuf>,
        /// Read source from stdin instead of file
        #[arg(long)]
        stdin: bool,
        /// Virtual file path for include resolution (used with --stdin)
        #[arg(long = "file-path")]
        file_path: Option<PathBuf>,
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
        /// Input WGSL file (omit when using --stdin)
        input: Option<PathBuf>,
        /// Output JSON file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Read source from stdin instead of file
        #[arg(long)]
        stdin: bool,
    },
    /// Minify a WGSL file
    Minify {
        /// Input WGSL file
        input: PathBuf,
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Enable dead code elimination
        #[arg(long)]
        dce: bool,
        /// Enable frequency-based identifier renaming
        #[arg(long)]
        rename: bool,
    },
    /// All-in-one: preprocess + validate + reflect (+ optional minify) and
    /// emit a single JSON `{ code, defines, reflection }` document. Build
    /// scripts use this to avoid orchestrating multiple wgslpp invocations.
    Pipeline {
        /// Input WGSL file
        #[arg(long)]
        input: PathBuf,
        /// wgslpp.json config (for package resolution); optional
        #[arg(long)]
        config: Option<PathBuf>,
        /// Named package: -P name=path
        #[arg(short = 'P', value_parser = parse_package)]
        packages: Vec<(String, PathBuf)>,
        /// Define: -D name or -D name=value
        #[arg(short = 'D', value_parser = parse_define)]
        defines: Vec<(String, String)>,
        /// Output JSON file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Write source map to file
        #[arg(long = "source-map")]
        source_map: Option<PathBuf>,
        /// Skip validation (still parses; reflection requires a parsed module)
        #[arg(long = "no-validate")]
        no_validate: bool,
        /// Minify the embedded `code` field via the naga WGSL writer
        #[arg(long)]
        minify: bool,
        /// Eliminate dead code before minify (no-op without --minify)
        #[arg(long)]
        dce: bool,
        /// Frequency-based identifier renaming before minify (no-op without --minify)
        #[arg(long)]
        rename: bool,
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
            config,
            stdin,
            file_path,
        } => cmd_preprocess::run(input, packages, defines, output, source_map, config, stdin, file_path),
        Commands::Validate {
            input,
            source_map,
            format,
        } => cmd_validate::run(input, source_map, format),
        Commands::Reflect { input, output, stdin } => cmd_reflect::run(input, output, stdin),
        Commands::Minify {
            input,
            output,
            dce,
            rename,
        } => cmd_minify::run(input, output, dce, rename),
        Commands::Pipeline {
            input,
            config,
            packages,
            defines,
            output,
            source_map,
            no_validate,
            minify,
            dce,
            rename,
        } => cmd_pipeline::run(
            input, config, packages, defines, output, source_map, no_validate, minify, dce, rename,
        ),
    };

    if let Err(e) = result {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}
