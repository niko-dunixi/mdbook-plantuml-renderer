extern crate crypto;

use std::fs::create_dir_all;
use std::io::{stderr, stdin, stdout, Read};
use std::path::{Path, PathBuf};

use clap::{App, Arg, ArgMatches, SubCommand};
use log::{debug, info, trace, warn};

use log::LevelFilter;
use mdbook::book::{Book, BookItem};
use mdbook::errors::Error;
use mdbook::preprocess::{CmdPreprocessor, Preprocessor, PreprocessorContext};

use markedit::{rewrite_between, Matcher, Rewriter};
use pulldown_cmark::{CodeBlockKind, CowStr, Event, LinkType, Parser, Tag};
use pulldown_cmark_to_cmark::cmark;

use crypto::digest::Digest;
use crypto::sha1::Sha1;

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
        let plantuml_build_directory = determine_plantuml_output_directory(&context);
        create_dir_all(&plantuml_build_directory)?;

        book.for_each_mut(|current_item: &mut BookItem| {
            if let BookItem::Chapter(ref mut current_chapter) = *current_item {
                info!("Working Chapter: {}", &current_chapter.name);

                let events_iterator = markedit::parse(&current_chapter.content);

                // let plantuml_renderer = create_render_plantuml_renderer(&plantuml_build_directory);
                let mutated_events_iterator = rewrite_between(
                    events_iterator,
                    renderable_plantuml_start,
                    renderable_plantuml_end,
                    |events: &mut Vec<Event<'_>>| {
                        // Intentionally consume and remove all events by mapping them into
                        // a single string of code. This helps strip out the opening/closing
                        // code-fences before and after the codeblock.
                        let plantuml_code = events
                            .iter()
                            .map(|e| match e {
                                Event::Text(plantuml_text) => plantuml_text.to_string(),
                                _ => "".into(),
                            })
                            .collect::<String>();
                        trace!("Found plantuml:\n{}", plantuml_code);

                        let mut hasher = Sha1::new();
                        hasher.input_str(&plantuml_code);
                        let plantuml_hash_sum = hasher.result_str();
                        debug!("Plantuml SHA1 hash sum: {}", &plantuml_hash_sum);
                        let mut plantuml_svg = PathBuf::new();
                        plantuml_svg.push(&plantuml_build_directory);
                        plantuml_svg.push(&plantuml_hash_sum);
                        plantuml_svg.set_extension("svg");
                        debug!("Filename: {}", plantuml_svg.to_str().unwrap());

                        let url = "https://upload.wikimedia.org/wikipedia/commons/thumb/3/3a/Cat03.jpg/1024px-Cat03.jpg\n";
                        let empty_str = "";
                        events.push(Event::Start(Tag::Image(
                            LinkType::Inline,
                            CowStr::Borrowed(url),
                            CowStr::Borrowed(empty_str),
                        )));
                        events.push(Event::End(Tag::Image(
                            LinkType::Inline,
                            CowStr::Borrowed(url),
                            CowStr::Borrowed(empty_str),
                        )));
                        events.remove(1);
                        events.push(Event::SoftBreak);
                    }
                );

                let mut content_buffer = String::with_capacity(current_chapter.content.len());
                current_chapter.content = cmark(mutated_events_iterator, &mut content_buffer, None)
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
