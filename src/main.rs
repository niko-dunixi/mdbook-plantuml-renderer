use std::io::{Read, stdin, stdout};
use std::path::{Path, PathBuf};
use std::fs::create_dir_all;
use mdbook::book::{Book, BookItem};
use mdbook::errors::Error;
use mdbook::preprocess::{CmdPreprocessor, Preprocessor, PreprocessorContext};
use clap::{App, Arg, ArgMatches, SubCommand};

fn main() -> Result<(), Error> {
    let matches = get_clap().get_matches();
    if let Some(_sub_args) = matches.subcommand_matches("supports") {
        return Ok(());
    }

    let std_in = stdin();
    let (ctx, book) = CmdPreprocessor::parse_input(std_in)?;
    let nop_preprocessor = Nop::default();
    let resulting_book = nop_preprocessor.run(&ctx, book)?;
    serde_json::to_writer(stdout(), &resulting_book)?;
    Ok(())
}

fn get_clap() -> App<'static, 'static> {
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    App::new("mdbook-plantuml-renderer")
        .version(VERSION)
        .author("Paul Freakn Baker")
        .about("A preprocessor that will replace some inline codeblocks with rendered PlantUML")
        // .arg(
        //     Arg::with_name("log")
        //         .short("l")
        //         .help("Log to './output.log' (may help troubleshooting rendering issues)."),
        // )
        .subcommand(
            SubCommand::with_name("supports")
                .arg(Arg::with_name("renderer").required(true))
                .about("Check whether a renderer is supported by this preprocessor"),
        )
}

#[derive(Default)]
pub struct Nop;

impl Preprocessor for Nop {
    fn name(&self) -> &str {
        "plantuml-renderer"
    }

    fn run(&self, context: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
        let plantuml_build_directory = determine_plantuml_output_directory(context);
        create_dir_all(plantuml_build_directory)?;

        book.for_each_mut(|current_item: &mut BookItem| {
            if let BookItem::Chapter(ref mut current_chapter) = *current_item {
                current_chapter.content = "Lol chapter modified".to_owned()
            }
        });
        Ok(book)
    }

    fn supports_renderer(&self, _renderer: &str) -> bool {
        true
    }
}

/// Takes the context root of the book and concatinates the build directory
/// and then appends the plantuml directory to that. This works because the build
/// directory is given to us relative to the 
fn determine_plantuml_output_directory(context: &PreprocessorContext) -> PathBuf {
    let mut plantuml_directory = PathBuf::new();
    plantuml_directory.push(&context.root);
    plantuml_directory.push(&context.config.build.build_dir);
    plantuml_directory.push("plantuml");
    plantuml_directory
}