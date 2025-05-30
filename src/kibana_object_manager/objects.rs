use eyre::{Result, eyre};
use jsrmx::{input::JsonReaderInput, output::JsonWritableOutput, processor::UnbundlerBuilder};
use owo_colors::OwoColorize;
use std::{path::PathBuf, str::FromStr};

pub fn bundle(file: &PathBuf, path: &PathBuf) -> Result<()> {
    log::info!(
        "Bundling saved objects from {} into {}",
        path.display().bright_black(),
        file.display().bright_black()
    );

    // jsrmx bundle "${import_dir}" "${import_file}" \
    //   --escape="attributes.panelsJSON,attributes.fieldFormatMap,attributes.controlGroupInput.ignoreParentSettingsJSON,attributes.controlGroupInput.panelsJSON,attributes.kibanaSavedObjectMeta.searchSourceJSON,attributes.optionsJSON,attributes.visState,attributes.fieldAttrs"
    Ok(())
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
    let unescape_fields = Some(vec![
        String::from("attributes.panelsJSON"),
        String::from("attributes.fieldFormatMap"),
        String::from("attributes.controlGroupInput.ignoreParentSettingsJSON"),
        String::from("attributes.controlGroupInput.panelsJSON"),
        String::from("attributes.kibanaSavedObjectMeta.searchSourceJSON"),
        String::from("attributes.optionsJSON"),
        String::from("attributes.visState"),
        String::from("attributes.fieldAttrs"),
    ]);

    UnbundlerBuilder::new(input, output)
        .type_field(type_field)
        .filename(filename)
        .drop_fields(drop_fields)
        .unescape_fields(unescape_fields)
        .build()
        .unbundle()
}
