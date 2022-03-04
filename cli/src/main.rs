use async_std::fs;
use clap::Clap;
use db::*;
use log::debug;
/// The CLI binary for invoking the compiler.
use std::path::PathBuf;
use std::sync::mpsc;

use walkdir::WalkDir;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

/// The canonical entry point for a program
const ENTRYPOINT_FILENAME: &'static str = "main.ws";

#[derive(Clap)]
struct Opts {
    #[clap(subcommand)]
    subcmd: Commands,
}

#[derive(Clap)]
enum Commands {
    Build(BuildOptions),
    Watch(WatchOptions),
}

#[derive(Clap)]
struct BuildOptions {
    #[clap(short, long)]
    path: String,
}

#[derive(Clap)]
struct WatchOptions {
    #[clap(short, long)]
    path: String,
}

fn resolve_path(path: &str) -> PathBuf {
    use std::fs::canonicalize;
    let path = PathBuf::from(path);
    canonicalize(path).expect("Unable to resolve provided path")
}

async fn build(options: BuildOptions) {
    let mut db = Database::default();
    let path = resolve_path(&options.path);
    let entry_point = path.join(ENTRYPOINT_FILENAME);
    let text = fs::read_to_string(entry_point.clone()).await.unwrap();
    db.set_file_text(entry_point.clone(), text.into());
    // Compile the entry point module so we can start building up
    // the import graph.
    let compiled = db.compile(entry_point.clone());
    // let ast = {
    //     let text = fs::read_to_string(entry_point.clone()).await.unwrap();
    //     db.set_file_text(entry_point.clone(), text.into());
    //     db.parse(entry_point.clone())
    // };
    match compiled {
        Ok(ast) => {
            debug!("ast: {:#?}", ast);
        }
        Err(error) => {
            let path_str = entry_point.to_str().unwrap_or("Unknown File");
            use diagnostics::error::{report_diagnostic_to_term, Error};
            if let Error::Diagnostic(diagnostic) = error {
                let source = db.file_text(entry_point.clone());
                report_diagnostic_to_term(diagnostic, path_str, &source);
            }
        }
    }
}

async fn watch(options: WatchOptions) {
    let mut db = Database::default();
    let root = resolve_path(&options.path);
    let entry_point = root.join(ENTRYPOINT_FILENAME);
    debug!("watching {:#?}", root);

    let text = fs::read_to_string(entry_point.clone()).await.unwrap();
    db.set_file_text(entry_point.clone(), text.into());

    // Compile the entry point module so we can start building up
    // the import graph.
    let _ = db.compile(entry_point.clone());

    for entry in
        WalkDir::new(&root)
            .into_iter()
            .filter_entry(|entry| match entry.path().extension() {
                Some(extension) => match extension.to_str() {
                    Some("ws") => true,
                    _ => false,
                },
                None => true,
            })
    {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            let text = fs::read_to_string(path).await.unwrap();
            let pathbuf = path.to_path_buf();
            db.set_file_text(pathbuf.clone(), text.into());
        }
    }

    let (tx, rx) = mpsc::channel();
    let mut watcher: RecommendedWatcher =
        Watcher::new_immediate(move |res| tx.send(res).unwrap()).unwrap();

    watcher
        .watch(root.to_str().unwrap(), RecursiveMode::Recursive)
        .unwrap();

    for res in rx {
        match res {
            Ok(event) => {
                use notify::event::{EventKind, ModifyKind};
                if let EventKind::Modify(modified) = event.kind {
                    if let ModifyKind::Data(_) = modified {
                        std::process::Command::new("clear").status().unwrap();
                        // Content of file has changed, recompile
                        let text = fs::read_to_string(entry_point.clone()).await.unwrap();
                        db.set_file_text(entry_point.clone(), text.into());
                        // Compile the entry point module so we can start building up
                        // the import graph.
                        let compiled = db.compile(entry_point.clone());
                        // let ast = {
                        //     let text = fs::read_to_string(entry_point.clone()).await.unwrap();
                        //     db.set_file_text(entry_point.clone(), text.into());
                        //     db.parse(entry_point.clone())
                        // };
                        match compiled {
                            Ok(_ast) => {
                                use std::io::Write;
                                use diagnostics::termcolor::{
                                    Color, ColorChoice, ColorSpec, StandardStream, WriteColor,
                                };
                                let mut stdout = StandardStream::stdout(ColorChoice::Always);
                                stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green))).unwrap();
                                writeln!(&mut stdout, "Compiled Successfully!").unwrap();
                            }
                            Err(error) => {
                                std::process::Command::new("clear").status().unwrap();
                                let path_str = entry_point.to_str().unwrap_or("Unknown File");
                                use diagnostics::error::{report_diagnostic_to_term, Error};
                                if let Error::Diagnostic(diagnostic) = error {
                                    let source = db.file_text(entry_point.clone());
                                    report_diagnostic_to_term(diagnostic, path_str, &source);
                                }
                            }
                        }
                    }
                }
            }
            Err(err) => println!("err: {:#?}", err),
        }
    }
}

#[async_std::main]
async fn main() {
    pretty_env_logger::init();
    let opts: Opts = Opts::parse();
    match opts.subcmd {
        Commands::Build(options) => build(options).await,
        Commands::Watch(options) => watch(options).await,
    }
}
