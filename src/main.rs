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

fn usage() {
    eprintln!(
        "Usage:\n  typikon check <schema.typ>\n  typikon compile <schema.typ> [--out-dir <directory>] [--target python,golang,typescript] [--public-format expanded|compact]"
    );
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
        usage();
        return ExitCode::from(2);
    };
    let Some(input) = args.next() else {
        usage();
        return ExitCode::from(2);
    };
    let mut out_dir = PathBuf::from(".");
    let mut targets = Vec::new();
    let mut public_format = PublicSchemaFormat::Expanded;
    while let Some(argument) = args.next() {
        if argument == "--out-dir" {
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
        "compile" => {
            if let Err(error) = fs::create_dir_all(&out_dir)
                .and_then(|_| {
                    fs::write(
                        out_dir.join(&artifacts.rust_file_name),
                        &artifacts.rust_source,
                    )
                })
                .and_then(|_| {
                    fs::write(
                        out_dir.join(&artifacts.public_schema_file_name),
                        &artifacts.public_schema,
                    )
                })
                .and_then(|_| {
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
            let selected = if targets.is_empty() {
                "rust".to_owned()
            } else {
                targets
                    .iter()
                    .map(|target| target.name())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            println!(
                "generated {}, {}; targets: {}",
                artifacts.rust_file_name, artifacts.public_schema_file_name, selected
            );
            ExitCode::SUCCESS
        }
        _ => {
            usage();
            ExitCode::from(2)
        }
    }
}
