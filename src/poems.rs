use anyhow::{Context, Result};
use log::debug;
use rand::seq::SliceRandom;
use serde::Deserialize;
use std::fs;
use std::path::Path;

const MAX_INSPIRATION_POEMS: usize = 50;

#[derive(Deserialize)]
struct Poem {
    canonical: PoemVersion,
}

#[derive(Deserialize)]
struct PoemVersion {
    text: String,
}

/// Select a random set of canonical poems to use as stylistic inspiration.
///
/// `None` means the configured directory did not contain any `.poem` files.
pub fn load_inspiration_poems(poetry_dir: &Path) -> Result<Option<String>> {
    debug!(
        "Scanning poetry directory {:?} for .poem files.",
        poetry_dir
    );
    let mut poems = Vec::new();

    for entry in fs::read_dir(poetry_dir)
        .with_context(|| format!("Could not read poetry directory {poetry_dir:?}"))?
    {
        let path = entry
            .with_context(|| format!("Could not read an entry in poetry directory {poetry_dir:?}"))?
            .path();

        if path
            .extension()
            .is_some_and(|extension| extension == "poem")
        {
            debug!("Reading inspiration poem {:?}.", path);
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("Could not read poem file {path:?}"))?;
            let poem: Poem = serde_yaml::from_str(&contents)
                .with_context(|| format!("Could not parse poem file {path:?}"))?;
            poems.push(poem.canonical.text);
        }
    }

    let mut rng = rand::rng();
    poems.shuffle(&mut rng);
    let selected_count = poems.len().min(MAX_INSPIRATION_POEMS);

    let inspiration = poems
        .into_iter()
        .take(MAX_INSPIRATION_POEMS)
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    debug!(
        "Loaded {} inspiration poems; selected up to {}; combined text is {} characters.",
        selected_count,
        MAX_INSPIRATION_POEMS,
        inspiration.len()
    );

    Ok((!inspiration.is_empty()).then_some(inspiration))
}
