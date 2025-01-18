use clap::{Args, Parser, Subcommand};
use epub::doc::EpubDoc;
use html5ever::tendril::TendrilSink;
use html5ever::tree_builder::TreeBuilderOpts;
use html5ever::{parse_document, ParseOpts};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::generation::options::GenerationOptions;
use ollama_rs::Ollama;
use std::io::Cursor;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

#[derive(Parser)]
#[clap(version, about, long_about = None)]
struct Cli {
    /// URL that points to Ollama
    #[arg(short, long, default_value = "http://localhost")]
    url: String,
    /// Port of Ollama instance
    #[arg(short, long, default_value = "11434")]
    port: u16,
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
    /// Tags to check for content
    #[arg(short, long, default_value = "p", value_delimiter = ',')]
    tags: Vec<String>,
    /// Tags to blacklist
    #[arg(short, long, default_value = "rt", value_delimiter = ',')]
    blacklist: Vec<String>,
    /// How many post-translation passes to do
    #[arg(long, default_value = "1")]
    passes: usize,
}

async fn translate(
    ollama: &Ollama,
    text: &str,
    language: &str,
    model: String,
) -> ollama_rs::error::Result<String> {
    ollama
        .generate(
            GenerationRequest::new(
                model,
                format!(
                    "Translate the following text to {language}:\n{text}\n{language}:"
                ),
            ).system(String::from("You are a professional translator. Don't answer with any notes. Answer only with the translation. Don't add the original. Follow the original style. Don't translate links.")
            ).options(GenerationOptions::default().temperature(0.4)))
        .await.and_then(|res| Ok(res.response))
}

async fn post_translation(
    ollama: &Ollama,
    original: &str,
    translated: &str,
    language: &str,
    model: String,
) -> ollama_rs::error::Result<String> {
    ollama
        .generate(GenerationRequest::new(model, format!("Given the original text, fix the {language} translated version.\nOriginal: {original}\nTranslated: {translated}"))
        .system(String::from("You are a professional translator. Fix the issues with this translation. Write only the translated sentence with the fixes. If there aren't errors, copy the translated sentence. Don't write anything else. Follow the original style.")).options(GenerationOptions::default().temperature(0.1))).await.and_then(|res| Ok(res.response))
}

fn traverse(
    handle: &Handle,
    good: bool,
    string: &mut String,
    tags: &[String],
    blacklist: &[String],
) {
    let mut is_good = false;
    let mut blacklisted = false;
    match &handle.data {
        NodeData::Element { name, .. } => {
            for tag in tags {
                is_good |= &name.local == tag;
            }
            for tag in blacklist {
                if &name.local == tag {
                    blacklisted = true;
                }
            }
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
        traverse(
            child,
            (good || is_good) && !blacklisted,
            string,
            tags,
            blacklist,
        );
    }
}

#[tokio_macros::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();
    if let Some(command) = cli.command {
        match command {
            Commands::List => {
                let ollama = Ollama::new(cli.url, cli.port);
                println!("{:?}", ollama.list_local_models().await.unwrap_or(vec![]));
            }
            Commands::Translate(args) => {
                let ollama = Ollama::new(cli.url, cli.port);
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
                        traverse(&dom.document, false, &mut text, &args.tags, &args.blacklist);
                    }
                    let parts = text.split("\n");
                    let mut file = tokio::fs::File::create(args.output)
                        .await
                        .expect("can't create output file");
                    let count = parts.clone().count();
                    for (idx, part) in parts.enumerate() {
                        if part.trim().len() < 1 {
                            continue;
                        }
                        let translation = tokio::time::timeout(
                            Duration::from_secs(20),
                            translate(&ollama, part, &args.language, args.model.clone()),
                        )
                        .await;
                        if let Ok(Ok(translation)) = translation {
                            let mut post = translation.clone();
                            for _ in 0..args.passes {
                                if let Ok(Ok(p)) = tokio::time::timeout(
                                    Duration::from_secs(20),
                                    post_translation(
                                        &ollama,
                                        part,
                                        &post,
                                        &args.language,
                                        args.model.clone(),
                                    ),
                                )
                                .await
                                {
                                    post = p;
                                } else {
                                    eprintln!("Error happened in post translation");
                                    return;
                                }
                            }

                            file.write_all(format!("{post}\n").as_bytes())
                                .await
                                .expect("can't write to output file");
                            println!(
                                "\n{idx}/{count}, {:2.2}%",
                                (idx as f32 / count as f32) * 100.0
                            );
                            println!("{part}");
                            println!("{post}");
                        } else {
                            eprintln!("Error happened");
                            return;
                        }
                    }
                }
            }
        }
    }
}
