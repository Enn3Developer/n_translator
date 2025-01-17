use clap::{Args, Parser, Subcommand};
use epub::doc::EpubDoc;
use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::Ollama;
use scraper::selectable::Selectable;
use scraper::{Element, Html, Selector};

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
            ).system(String::from("You are a professional translator. Don't answer with any notes. Answer only with the translation. Don't add the original. Follow the original style")))
        .await.and_then(|res| Ok(res.response))
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
                    epub.set_current_page(20);
                    let fragment = Html::parse_fragment(
                        epub.get_current_str()
                            .unwrap_or((String::new(), String::new()))
                            .0
                            .as_str(),
                    );
                    let selector = Selector::parse("p").unwrap();
                    let span_selector = Selector::parse("span").unwrap();
                    for element in fragment.select(&selector) {
                        if element.inner_html().contains("br") {
                            println!("line break found");
                        }
                        for span in element.select(&span_selector) {
                            println!(
                                "{}: {}",
                                span.inner_html(),
                                span.inner_html().contains("<span>"),
                            );
                        }
                    }
                    // println!("{:?}", epub.get_current_str());
                    // if let Ok(response) =
                    //     translate(ollama, args.file, args.language, args.model).await
                    // {
                    //     println!("{}", response);
                    // }
                }
            }
        }
    }
}
