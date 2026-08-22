use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};
use typikon::{PublicSchemaFormat, compile_schema_with_format};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Python,
    Go,
    TypeScript,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GenerateMode {
    Backend,
    Public,
    All,
}

impl Target {
    fn name(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::Go => "golang",
            Self::TypeScript => "typescript",
        }
    }
}

fn parse_target(value: &str) -> Option<Target> {
    match value {
        "python" => Some(Target::Python),
        "go" | "golang" => Some(Target::Go),
        "typescript" | "ts" => Some(Target::TypeScript),
        _ => None,
    }
}

fn add_targets(value: &str, targets: &mut Vec<Target>) -> Result<(), String> {
    for name in value.split(',').map(str::trim) {
        if name.is_empty() {
            return Err("empty target".into());
        }
        if name == "rust" {
            continue;
        }
        let Some(target) = parse_target(name) else {
            return Err(format!("unknown target: {name}"));
        };
        if !targets.contains(&target) {
            targets.push(target);
        }
    }
    Ok(())
}

fn parse_generate_mode(value: &str) -> Option<GenerateMode> {
    match value {
        "backend" | "backends" => Some(GenerateMode::Backend),
        "public" => Some(GenerateMode::Public),
        "all" => Some(GenerateMode::All),
        _ => None,
    }
}

fn print_help() {
    println!(
        "Typikon — schema compiler for the Typikon binary wire format\n\n\
USAGE:\n  typikon <COMMAND> [OPTIONS]\n\n\
COMMANDS:\n  check <SCHEMA>      Validate a schema without writing files\n  generate <KIND>     Generate backend, public, or all artifacts\n  help                Show this help\n\n\
GENERATE KINDS:\n  backend             Rust and selected language backends only\n  public              Public .typ schema only\n  all                 Backends and public schema\n\n\
OPTIONS:\n  --out-dir <DIR>     Output directory (default: current directory)\n  --target <LIST>     Add language backends: python, golang, typescript\n  --public-format <F> expanded (default) or compact\n  -h, --help          Show command help\n\n\
EXAMPLES:\n  typikon check examples/messenger.typ\n  typikon generate backend examples/messenger.typ --target python,golang,typescript\n  typikon generate public examples/messenger.typ --out-dir /tmp/public\n  typikon generate all examples/messenger-10.public.typ --out-dir /tmp/all\n"
    );
}

fn print_command_help(command: &str) {
    match command {
        "check" => println!(
            "Validate a Typikon schema.\n\nUSAGE:\n  typikon check <SCHEMA>\n\nEXAMPLE:\n  typikon check examples/messenger.typ"
        ),
        "generate" => println!(
            "Generate selected Typikon artifacts.\n\nUSAGE:\n  typikon generate <KIND> <SCHEMA> [OPTIONS]\n\nKINDS:\n  backend             Rust and selected language backends only\n  public              Public .typ schema only\n  all                 Backend and public artifacts\n\nOPTIONS:\n  --out-dir <DIR>     Output directory (default: current directory)\n  --target <LIST>     python, golang, or typescript (backend/all)\n  --public-format <F> expanded (default) or compact (public/all)\n  -h, --help          Show this help\n"
        ),
        _ => print_help(),
    }
}

fn parse_public_format(value: &str) -> Option<PublicSchemaFormat> {
    match value {
        "expanded" => Some(PublicSchemaFormat::Expanded),
        "compact" => Some(PublicSchemaFormat::Compact),
        _ => None,
    }
}

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_help();
        return ExitCode::from(2);
    };
    if matches!(command.as_str(), "-h" | "--help" | "help") {
        print_help();
        return ExitCode::SUCCESS;
    }
    if matches!(command.as_str(), "-V" | "--version" | "version") {
        println!("typikon {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    if !matches!(command.as_str(), "check" | "generate") {
        eprintln!("unknown command: {command}\n");
        print_help();
        return ExitCode::from(2);
    }
    let mode = if command == "generate" {
        let Some(kind) = args.next() else {
            print_command_help("generate");
            return ExitCode::from(2);
        };
        if matches!(kind.as_str(), "-h" | "--help") {
            print_command_help("generate");
            return ExitCode::SUCCESS;
        }
        let Some(mode) = parse_generate_mode(&kind) else {
            eprintln!("unknown generate kind: {kind} (expected backend, public, or all)\n");
            print_command_help("generate");
            return ExitCode::from(2);
        };
        Some(mode)
    } else {
        None
    };
    let Some(input) = args.next() else {
        print_command_help(&command);
        return ExitCode::from(2);
    };
    if matches!(input.as_str(), "-h" | "--help") {
        print_command_help(&command);
        return ExitCode::SUCCESS;
    }
    let mut out_dir = PathBuf::from(".");
    let mut targets = Vec::new();
    let mut public_format = PublicSchemaFormat::Expanded;
    while let Some(argument) = args.next() {
        if matches!(argument.as_str(), "-h" | "--help") {
            print_command_help(&command);
            return ExitCode::SUCCESS;
        } else if argument == "--out-dir" {
            let Some(path) = args.next() else {
                eprintln!("missing value for --out-dir");
                return ExitCode::from(2);
            };
            out_dir = PathBuf::from(path);
        } else if argument == "--target" {
            let Some(value) = args.next() else {
                eprintln!("missing value for --target");
                return ExitCode::from(2);
            };
            if let Err(error) = add_targets(&value, &mut targets) {
                eprintln!("{error}");
                return ExitCode::from(2);
            }
        } else if argument == "--public-format" {
            let Some(value) = args.next() else {
                eprintln!("missing value for --public-format");
                return ExitCode::from(2);
            };
            let Some(format) = parse_public_format(&value) else {
                eprintln!("unknown public format: {value} (expected expanded or compact)");
                return ExitCode::from(2);
            };
            public_format = format;
        } else {
            eprintln!("unknown argument: {argument}");
            return ExitCode::from(2);
        }
    }
    let input_path = Path::new(&input);
    let source = match fs::read_to_string(input_path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("cannot read {input}: {error}");
            return ExitCode::from(1);
        }
    };
    let name = input_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&input);
    let artifacts = match compile_schema_with_format(&source, name, public_format) {
        Ok(artifacts) => artifacts,
        Err(error) => {
            eprintln!("{} at byte {}", error.message, error.position);
            return ExitCode::from(1);
        }
    };
    match command.as_str() {
        "check" => {
            println!("valid Layer {}: {}", artifacts.layer, name);
            ExitCode::SUCCESS
        }
        "generate" => {
            let mode = mode.expect("generation mode is set for generate");
            if mode == GenerateMode::Public && !targets.is_empty() {
                eprintln!("--target cannot be used with public-only generation");
                return ExitCode::from(2);
            }
            if let Err(error) = fs::create_dir_all(&out_dir)
                .and_then(|_| {
                    if mode != GenerateMode::Public {
                        fs::write(
                            out_dir.join(&artifacts.rust_file_name),
                            &artifacts.rust_source,
                        )
                    } else {
                        Ok(())
                    }
                })
                .and_then(|_| {
                    if mode != GenerateMode::Backend {
                        fs::write(
                            out_dir.join(&artifacts.public_schema_file_name),
                            &artifacts.public_schema,
                        )
                    } else {
                        Ok(())
                    }
                })
                .and_then(|_| {
                    if mode == GenerateMode::Public {
                        return Ok(());
                    }
                    for target in &targets {
                        match target {
                            Target::Python => {
                                fs::write(
                                    out_dir.join(&artifacts.bridge_file_names[0]),
                                    &artifacts.bridge_sources[0],
                                )?;
                                fs::write(
                                    out_dir.join(&artifacts.python_file_name),
                                    &artifacts.python_source,
                                )?;
                            }
                            Target::Go => {
                                fs::write(
                                    out_dir.join(&artifacts.bridge_file_names[1]),
                                    &artifacts.bridge_sources[1],
                                )?;
                                fs::write(
                                    out_dir.join(&artifacts.go_file_name),
                                    &artifacts.go_source,
                                )?;
                                fs::write(
                                    out_dir.join(&artifacts.bridge_header_name),
                                    &artifacts.bridge_header,
                                )?;
                            }
                            Target::TypeScript => {
                                fs::write(
                                    out_dir.join(&artifacts.bridge_file_names[2]),
                                    &artifacts.bridge_sources[2],
                                )?;
                                fs::write(
                                    out_dir.join(&artifacts.typescript_file_name),
                                    &artifacts.typescript_source,
                                )?;
                            }
                        }
                    }
                    Ok(())
                })
            {
                eprintln!("cannot write generated artifacts: {error}");
                return ExitCode::from(1);
            }
            let selected = if mode == GenerateMode::Public {
                "public".to_owned()
            } else if targets.is_empty() {
                "rust".to_owned()
            } else {
                targets
                    .iter()
                    .map(|target| target.name())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let files = match mode {
                GenerateMode::Backend => artifacts.rust_file_name.clone(),
                GenerateMode::Public => artifacts.public_schema_file_name.clone(),
                GenerateMode::All => format!(
                    "{}, {}",
                    artifacts.rust_file_name, artifacts.public_schema_file_name
                ),
            };
            println!("generated {}; targets: {}", files, selected);
            ExitCode::SUCCESS
        }
        _ => {
            print_help();
            ExitCode::from(2)
        }
    }
}
