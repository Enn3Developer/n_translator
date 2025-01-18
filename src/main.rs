use clap::{Args, Parser, Subcommand};
use epub::doc::EpubDoc;
use html5ever::tendril::TendrilSink;
use html5ever::tree_builder::TreeBuilderOpts;
use html5ever::{parse_document, ParseOpts};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::Ollama;
use std::io::Cursor;

#[derive(Parser)]
#[clap(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Lists all available Ollama models
    List,
    /// Translates a book
    Translate(TranslateArgs),
}

#[derive(Args)]
struct TranslateArgs {
    /// File of the book to translate
    #[arg(short, long)]
    file: String,
    /// Where to save the translated book
    #[arg(short, long)]
    output: String,
    /// Model to use
    #[arg(short, long, default_value = "thinkverse/towerinstruct")]
    model: String,
    /// Language to translate to
    #[arg(short, long, default_value = "English")]
    language: String,
}

async fn translate(
    ollama: Ollama,
    text: String,
    language: String,
    model: String,
) -> ollama_rs::error::Result<String> {
    ollama
        .generate(
            GenerationRequest::new(
                model,
                format!(
                    "Translate the following text to {}:\n{}\n{}:",
                    language, text, language
                ),
            ).system(String::from("You are a professional translator. Don't answer with any notes. Answer only with the translation. Don't add the original. Follow the original style. Don't translate links.")))
        .await.and_then(|res| Ok(res.response))
}

fn traverse(handle: &Handle, good: bool, string: &mut String) {
    let mut is_good = false;
    match &handle.data {
        NodeData::Element { name, .. } => {
            is_good = &name.local == "p";
        }
        NodeData::Text { contents } => {
            let str = contents.borrow().to_string();
            if good || str.contains("\n") {
                string.push_str(&str);
            }
        }
        _ => {}
    }
    for child in handle.children.borrow().iter() {
        traverse(child, good || is_good, string);
    }
}

#[tokio_macros::main(flavor = "multi_thread")]
async fn main() {
    let cli = Cli::parse();
    if let Some(command) = cli.command {
        match command {
            Commands::List => {
                let ollama = Ollama::default();
                println!("{:?}", ollama.list_local_models().await.unwrap_or(vec![]));
            }
            Commands::Translate(args) => {
                let ollama = Ollama::default();
                if let Ok(mut epub) = EpubDoc::new(&args.file) {
                    let pages = epub.get_num_pages();
                    let mut text = String::new();
                    for page in 0..pages {
                        epub.set_current_page(page);
                        let mut current = epub.get_current().unwrap();
                        let mut cursor = Cursor::new(&mut current.0);
                        let dom = parse_document(
                            RcDom::default(),
                            ParseOpts {
                                tree_builder: TreeBuilderOpts {
                                    drop_doctype: true,
                                    ..Default::default()
                                },
                                ..Default::default()
                            },
                        )
                        .from_utf8()
                        .read_from(&mut cursor)
                        .unwrap();
                        traverse(&dom.document, false, &mut text);
                    }
                    println!("{text:?}");
                }
            }
        }
    }
}
