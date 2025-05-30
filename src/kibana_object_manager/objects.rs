use eyre::{OptionExt, Result, eyre};
use jsrmx::{
    input::{InputDirectory, JsonReaderInput},
    output::{JsonAppendableOutput, JsonWritableOutput},
    processor::{BundlerBuilder, UnbundlerBuilder},
};
use owo_colors::OwoColorize;
use std::{path::PathBuf, str::FromStr};

fn escape_fields() -> Option<Vec<String>> {
    Some(vec![
        String::from("attributes.panelsJSON"),
        String::from("attributes.fieldFormatMap"),
        String::from("attributes.controlGroupInput.ignoreParentSettingsJSON"),
        String::from("attributes.controlGroupInput.panelsJSON"),
        String::from("attributes.kibanaSavedObjectMeta.searchSourceJSON"),
        String::from("attributes.optionsJSON"),
        String::from("attributes.visState"),
        String::from("attributes.fieldAttrs"),
    ])
}

pub fn bundle(file: &PathBuf, path: &PathBuf) -> Result<usize> {
    log::info!(
        "Bundling saved objects from {} into {}",
        path.display().bright_black(),
        file.display().bright_black()
    );

    let input = InputDirectory::new(path.clone().join("objects"));
    let file = file.to_str().ok_or_eyre("Invalid bundler file path")?;
    let output = JsonAppendableOutput::from_str(file)?;

    BundlerBuilder::new(input, output)
        .escape_fields(escape_fields())
        .build()
        .bundle()?;

    let line_count = std::fs::read_to_string(file)?.lines().count();

    Ok(line_count)
}

pub fn unbundle(file: &PathBuf, path: &PathBuf) -> Result<()> {
    let export_str = file
        .to_str()
        .ok_or_else(|| eyre!("Failed to convert export path to string"))?;
    let input = JsonReaderInput::from_str(export_str)?;
    let output_str = path.join("objects");
    let output_str = output_str
        .to_str()
        .expect("Failed to convert output path to string");
    let output = JsonWritableOutput::from_str(output_str).map_err(|e| eyre!(e.to_string()))?;
    output
        .write()
        .expect("Error acquiring write lock on output")
        .set_pretty(true);
    let filename = Some(vec![
        String::from("attributes.title"),
        String::from("attributes.name"),
    ]);
    let type_field = Some(String::from("type"));
    let drop_fields = Some(vec![
        String::from("created_at"),
        String::from("created_by"),
        String::from("count"),
        String::from("managed"),
        String::from("updated_at"),
        String::from("updated_by"),
        String::from("version"),
    ]);

    UnbundlerBuilder::new(input, output)
        .type_field(type_field)
        .filename(filename)
        .drop_fields(drop_fields)
        .unescape_fields(escape_fields())
        .build()
        .unbundle()
}
