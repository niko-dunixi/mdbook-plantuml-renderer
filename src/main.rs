use std::fs::create_dir_all;
use std::io::{stderr, stdin, stdout, Read};
use std::path::{Path, PathBuf};

use clap::{App, Arg, ArgMatches, SubCommand};
use log::{info, trace, warn};

use log::LevelFilter;
use mdbook::book::{Book, BookItem};
use mdbook::errors::Error;
use mdbook::preprocess::{CmdPreprocessor, Preprocessor, PreprocessorContext};

use markedit::{rewrite_between, Matcher, Rewriter};
use pulldown_cmark::{CodeBlockKind, CowStr, Event, LinkType, Parser, Tag};
use pulldown_cmark_to_cmark::cmark;
// use markedit::rewriter::{rewrite_between};

static PLANTUML_RENDERABLE_LANGUAGE: &str = "plantuml,render";

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
struct plantuml_renderer_preprocessor {}

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

                let mutated_events: Vec<_> = rewrite_between(
                    events_iterator,
                    renderable_plantuml_start,
                    renderable_plantuml_end,
                    uppercase_all_text,
                ).collect();

                let mut content_buffer = String::with_capacity(current_chapter.content.len());
                current_chapter.content = cmark(mutated_events.iter(), &mut content_buffer, None)
                    .map(|_| content_buffer)
                    .map_err(|err| Error::from(format!("Markdown serialization failed: {}", err)))
                    .unwrap();
            }
        });
        Ok(book)
    }

    fn supports_renderer(&self, _renderer: &str) -> bool {
        true
    }
}

fn renderable_plantuml_start(event: &Event<'_>) -> bool {
    match event {
        Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(language))) => {
            language.to_string() == PLANTUML_RENDERABLE_LANGUAGE.to_string()
        }
        _ => false,
    }
}

fn renderable_plantuml_end(event: &Event<'_>) -> bool {
    match event {
        Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(language))) => {
            language.to_string() == PLANTUML_RENDERABLE_LANGUAGE.to_string()
        }
        _ => false,
    }
}

fn uppercase_all_text<'src>(events: &mut Vec<Event<'src>>) {
    for event in events {
        if let Event::Text(ref mut text) = event {
            *text = text.to_uppercase().into();
        }
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
