use clap::{Args, Parser, Subcommand};
use ollama_rs::Ollama;

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
            Commands::Translate(args) => {}
        }
    }
}
