use std::time::Instant;

use color_eyre::eyre;
use dialoguer::Input;
use tracing::{trace, warn};

use indexing::{normalise, tokenise, Indexer, LiveIndexer};

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt::try_init().map_err(|e| eyre::eyre!(e))?;

    let indexer =
        Indexer::new(|| Box::new(tokenise::RegexTokeniser::new(r"[^\w-]+").unwrap()) as _)
            .with_normaliser(normalise::Unicode::NFC)
            .with_normaliser(normalise::LowerCase)
            .with_normaliser(normalise::StopWords::new(&["a", "the", "and", "or", "not"]));

    let indexer = LiveIndexer::start(indexer)?;

    loop {
        let input: String = Input::new().interact()?;

        if let Some(command) = input.strip_prefix("/") {
            let items = command.split_whitespace().collect::<Vec<_>>();

            match items.as_slice() {
                [] => (),
                ["quit", ..] => return Ok(()),
                ["watch", paths @ ..] => paths.iter().try_for_each(|path| indexer.watch(path))?,
                ["unwatch", paths @ ..] => {
                    match paths.iter().try_for_each(|path| indexer.unwatch(path)) {
                        Ok(_) => (),
                        Err(e) => warn!(error = %e, "failed to unwatch"),
                    }
                }
                _ => println!("unrecognised command: {}", items.join(" ")),
            }

            continue;
        }

        let start = Instant::now();
        let items = indexer
            .query(&input)
            .into_iter()
            .map(|path| format!(" - {}", path))
            .collect::<Vec<_>>();
        println!(" :: {} matches:\n{}", items.len(), items.join("\n"));

        trace!(term = ?input, duration = ?start.elapsed(), "query executed");
    }
}
