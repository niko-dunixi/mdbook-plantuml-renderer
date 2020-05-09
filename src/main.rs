use std::io::{Read, stdin, stdout, stderr};
use std::path::{Path, PathBuf};
use std::fs::create_dir_all;

use log::{info, trace, warn};
use clap::{App, Arg, ArgMatches, SubCommand};

use mdbook::book::{Book, BookItem};
use mdbook::errors::Error;
use mdbook::preprocess::{CmdPreprocessor, Preprocessor, PreprocessorContext};
use log::LevelFilter;

use pulldown_cmark::{CowStr, Parser, Event, Tag, CodeBlockKind, LinkType};
use pulldown_cmark_to_cmark::cmark;
use markedit::{Matcher, Rewriter};

fn main() -> Result<(), Box<std::error::Error>> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} [{}] ({}): {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .chain(stderr())
        // .chain(fern::log_file("output.log")?)
        .apply()?;

    let preprocessor = plantuml_renderer_preprocessor::default();
    let matches = get_clap().get_matches();
    if let Some(support_subcommand) = matches.subcommand_matches("supports") {
        // if let Some(renderer_argument) = support_subcommand.args.get("renderer") {
        //     // preprocessor.supports_renderer(renderer_argument as String);
        // }
        return Ok(());
    }

    let (context, book) = CmdPreprocessor::parse_input(stdin())?;
    // simple_logging::log_to_stderr(LevelFilter::Info);

    info!("PlantUML Preprocessor initiated");

    let resulting_book = preprocessor.run(&context, book)?;
    serde_json::to_writer(stdout(), &resulting_book)?;
    Ok(())
}

fn get_clap() -> App<'static, 'static> {
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    App::new("mdbook-plantuml-renderer")
        .version(VERSION)
        .author("Paul Freakn Baker")
        .about("A preprocessor that will replace some inline codeblocks with rendered PlantUML")
        .subcommand(
            SubCommand::with_name("supports")
                .arg(Arg::with_name("renderer").required(true))
                .about("Check whether a renderer is supported by this preprocessor"),
        )
}

#[derive(Default)]
struct plantuml_renderer_preprocessor {
}

impl Preprocessor for plantuml_renderer_preprocessor {
    fn name(&self) -> &str {
        "plantuml-renderer"
    }

    fn run(&self, context: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
        let plantuml_build_directory = determine_plantuml_output_directory(context);
        create_dir_all(plantuml_build_directory)?;

        book.for_each_mut(|current_item: &mut BookItem| {
            if let BookItem::Chapter(ref mut current_chapter) = *current_item {
                info!("Working Chapter: {}", current_chapter.name);

                let events_iterator = markedit::parse(&current_chapter.content);

                let matcher = plantuml_codeblock_matcher::default().falling_edge();
                let rewriter = markedit::insert_markdown_before("(look, plantuml lol)", matcher);

                let mutated_events: Vec<_> = markedit::rewrite(events_iterator, rewriter).collect();

                let mut content_buffer = String::with_capacity(current_chapter.content.len());
                current_chapter.content = cmark(mutated_events.iter(), &mut content_buffer, None)
                    .map(|_| content_buffer)
                    .map_err(|err| {
                        Error::from(format!("Markdown serialization failed: {}", err))
                    })
                    .unwrap();
            }
        });
        Ok(book)
    }

    fn supports_renderer(&self, _renderer: &str) -> bool {
        true
    }
}

#[derive(Default)]
struct plantuml_codeblock_matcher {
    is_plantuml: bool,
}

impl Matcher for plantuml_codeblock_matcher {
    fn matches_event(&mut self, event: &pulldown_cmark::Event<'_>) -> bool {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(codeblock_language))) if self.is_renderable_plantuml(codeblock_language) => {
                info!("Found renderable plantuml start");
                self.is_plantuml = true;
            },
            Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(codeblock_language))) if self.is_renderable_plantuml(codeblock_language) => {
                info!("Found renderable plantuml end");
                self.is_plantuml = false;
                return true
            }
            _ => {},
        }
        self.is_plantuml
    }
}

impl plantuml_codeblock_matcher {
    fn is_renderable_plantuml(&self, value: &pulldown_cmark::CowStr<>) -> bool{
        value.to_string() == "plantuml,render".to_string()
    }
}

/// Takes the context root of the book and concatinates the build directory.
/// This works because the build directory is given to us relative to the
/// project root
fn determine_build_directory(context: &PreprocessorContext) -> PathBuf {
    let mut build_directory = PathBuf::from(&context.root.as_path());
    build_directory.push(&context.config.build.build_dir);
    build_directory
}

fn determine_plantuml_output_directory(context: &PreprocessorContext) -> PathBuf {
    let mut plantuml_directory = determine_build_directory(context);
    plantuml_directory.push("plantuml-renderer");
    plantuml_directory
}
