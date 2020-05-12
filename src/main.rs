extern crate crypto;

use std::fs::{create_dir_all, File};
use std::io::{stderr, stdin, stdout, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

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

    let preprocessor = PlantumlRendererPreprocessor::default();
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
struct PlantumlRendererPreprocessor {}

impl Preprocessor for PlantumlRendererPreprocessor {
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
                        events.clear();
                        trace!("Found plantuml:\n{}", plantuml_code);
                        // Generate the SHA sum. This lets us be lazy. If the diagram already exists
                        // it doesn't need to be re-created, merely referenced.
                        let mut hasher = Sha1::new();
                        hasher.input_str(&plantuml_code);
                        let plantuml_hash_sum = hasher.result_str();
                        debug!("Plantuml SHA1 hash sum: {}", &plantuml_hash_sum);
                        let mut plantuml_svg_filename = PathBuf::new();
                        plantuml_svg_filename.push(&plantuml_build_directory);
                        plantuml_svg_filename.push(&plantuml_hash_sum);
                        plantuml_svg_filename.set_extension("svg");
                        debug!("Filename: {}", plantuml_svg_filename.to_str().unwrap());
                        // If the SVG doesn't exist, dump the PUML file for plantuml to parse
                        if !&plantuml_svg_filename.exists() {
                            let mut puml_filename = PathBuf::new();
                            puml_filename.push(&plantuml_build_directory);
                            puml_filename.push(&plantuml_hash_sum);
                            puml_filename.set_extension("puml");
                            debug!(
                                "SVG doesn't exist, writing PUML data: {}",
                                puml_filename.to_str().unwrap()
                            );
                            let mut puml_file = File::create(&puml_filename).unwrap();
                            write!(puml_file, "{}", plantuml_code);
                            // Call plantuml and generate the SVG
                            let output = Command::new("/usr/local/bin/plantuml")
                                .arg("-tsvg")
                                .arg("-o")
                                .arg(&plantuml_build_directory.to_str().unwrap())
                                .arg(&puml_filename.to_str().unwrap())
                                .output()
                                .expect("Failed to run PlantUML");
                            debug!("PlantUML Exit Status: {}", output.status);
                            debug!(
                                "PlantUML stdout: {}",
                                String::from_utf8(output.stdout).unwrap()
                            );
                            debug!(
                                "PlantUML stderr: {}",
                                String::from_utf8(output.stderr).unwrap()
                            );
                        }
                        // Create the relative filename to use, and then place it programatically
                        // as an image to be re-introduced to the mdbook
                        let empty_str = "";
                        events.push(Event::Start(Tag::Image(
                            LinkType::Inline,
                            CowStr::Boxed(plantuml_svg_filename.to_str().unwrap().into()),
                            CowStr::Borrowed(empty_str),
                        )));
                        events.push(Event::End(Tag::Image(
                            LinkType::Inline,
                            CowStr::Boxed(plantuml_svg_filename.to_str().unwrap().into()),
                            CowStr::Borrowed(empty_str),
                        )));
                        events.push(Event::SoftBreak);
                    },
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
