use std::{env, fs, process};

use quantic_engine::{Engine, ENGINE_NAME, ENGINE_VERSION};

fn main() {
    let mut args = env::args().skip(1);
    let Some(input) = args.next() else {
        eprintln!("{ENGINE_NAME} {ENGINE_VERSION}");
        eprintln!("usage: qengine <file.html|https://site> [output.png]");
        process::exit(2);
    };
    let output_path = args
        .next()
        .unwrap_or_else(|| "quantic-render.png".to_string());

    let engine = Engine::new();
    let output = if input.starts_with("http://") || input.starts_with("https://") {
        match engine.load_interactive_url(&input) {
            Ok(page) => page.snapshot(),
            Err(error) => {
                eprintln!("cannot load interactive page {input}: {error}");
                process::exit(1);
            }
        }
    } else {
        let source = match fs::read_to_string(&input) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("cannot read {input}: {error}");
                process::exit(1);
            }
        };
        match engine.load_interactive_html(&source) {
            Ok(page) => page.snapshot(),
            Err(error) => {
                eprintln!("cannot execute interactive page {input}: {error}");
                process::exit(1);
            }
        }
    };

    if let Err(error) = engine.save_png(&output, &output_path) {
        eprintln!("cannot render {output_path}: {error}");
        process::exit(1);
    }

    println!("{ENGINE_NAME} {ENGINE_VERSION}");
    println!("DOM nodes: {}", output.document.nodes.len());
    println!("Display items: {}", output.display_list.len());
    println!("Decoded images: {}", output.decoded_images.len());
    println!("Content height: {} px", output.content_height);
    println!("Blocked resources: {}", output.blocked_resources.len());
    println!("Console messages: {}", output.console_messages.len());
    println!("Script errors: {}", output.script_errors.len());
    println!("Rendered: {output_path}");

    for blocked in &output.blocked_resources {
        println!("BLOCKED {blocked}");
    }
    for error in &output.script_errors {
        println!("SCRIPT ERROR {error}");
    }
}
