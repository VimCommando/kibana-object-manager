# Kibana Object Manager CLI Documentation

⚠️ This `bash` version has been superceded by the Rust rewrite. ⚠️

The complete _revival_ and **rewrite** of the Kibana Object Manager plugin.

## Quick Start
The intent of `kibob` is to provide a Git-inspired interface for managing Kibana saved objects.

### Dependencies
This script depends on a few other command-line utilities:
1. `jsrmx` - bundle and unbundle NDJSON: https://github.com/VimCommando/jsrmx
2. `curl` - make HTTP requests to Kibana: https://curl.se/
3. `jq` - format, parse, and update JSON: https://stedolan.github.io/jq/
4. `grep` - filtering and searching text: https://www.gnu.org/software/grep/

Almost every modern unix-based system has `grep` and `curl` installed. You'll find `jq` in every package repo. For `jsrmx` the easiest way is through Rust's `cargo` package manager at [https://doc.rust-lang.org/cargo/getting-started/installation.html](https://doc.rust-lang.org/cargo/getting-started/installation.html).

### Initial Repository Setup
1. Clone this repository
2. Symlink `kibob` to your `$PATH`
3. Download the `export.ndjson` from Kibana and copy it into a new folder
4. In the new folder, run `git init` to initialize a new Git repository
5. Run `kibob init` to slice up the `export.ndjson` into separate files with a `manifest.json`
6. Use `git add .` and `git commit -m "Initial commit"`

You're now tracking your Kibana objects with source control!

> ⚠️ This script will modify the `export.ndjson` file. Make sure you made a copy in step 3.

### Pull changes directly from Kibana
1. Copy the `dotenv/localhost` to a `.env` file in your new repo
2. Update with your Kibana URL and credentials
3. Use `kibob pull` to fetch updates to any objects listed in the `manifest.json`

Now use your favorite Git client to review the changes, add and commit as normal.

### Push changes back to Kibana
Any changes made in the repository can be pushed back to Kibana using `kibob push`. Where this is most useful is across different environments, such as development and production. Simply make a different `.env` file for each environment.

For example use the default `.env` for your development environment, and create an `.env.prod` for your production environment.

```sh
kibob pull
kibob push --env prod
```

This will pull all the objects listed in the `manifest.json` from your dev cluster and push them to your prod cluster! By default all objects are imported with the `managed: true` flag set, so no changes can be made directoy in production.

## CLI Reference
Kibana Object Manager: `--{kibob}->` is the Git-flavored side dish to prepare Kibana saved objects for version control!

### Usage
```
kibob <command> [options] <arguments>
```

### Commands
- `init`    Slice up an `export.ndjson` into objects files and create a `manifest.json`
- `auth`    Test authorization to a Kibana remote
- `pull`    Fetch saved objects from a Kibana remote
- `push`    Update saved objects in a Kibana remote
- `add`     Add saved objects to the manifest
- `togo`    Order your Kibana objects to go! (bundle an NDJSON for distribution)
- `help`    Get detailed help, use a command name as the argument

### Global Options
- `-e, --env <NAME|FILE>` - The `.env.NAME` or `FILE` file to source credentials from (default `.env`)
- `-s, --space <ID>`      - Kibana space id to use (default `default`)
- `--debug`               - More verbose logging and retention of temporary files

## Add Command

```
kibob add [output_dir]
```

Add an object to the menu, err, repository. Exports saved objects by ID, including related objects. Adds entries to the `[output_dir]/manifest.json` and moves objects into `<output_dir>/objects/*.json`

**Options:**
- `-o, --objects <IDS>` - Comma-separated list of `"type=uuid"` objects to export from Kibana
- `-f, --file <FILE>`   - Filename of an `export.ndjson` to merge into existing manifest

**Arguments:**
- `[output_dir]`        - Directory to save the exported objects to. Must contain a `manifest.json` file. (default `.`)

## Auth command
```
kibob auth
```

Tests the Kibana authorization configuration

## Init command
```
kibob init [export] [manifest_file]
```

Initializes a Kibana object repository from an `export.ndjson`.

**Arguments:**
- `[export]`            - An NDJSON file or directory with an `export.ndjson` to build a manifest file from (default: `export.ndjson`)
- `[manifest_file]`     - The manifest file to generate (default: `manifest.json`)

## Pull Command
```
kibob pull [output_dir]
```

Export and unbundle the Kibana saved objects listed in `[output_dir]/manifest.json` into `[output_dir]/objects/*.json` objects.

**Arguments:**
- `[output_dir]`        - Directory to save exported objects to. Must contain a `manifest.json` file.

### Push Command
```
kibob push [input_dir]
```

Bundle up the `[input_dir]/objects/*.json` objects to go and deliver them to Kibana!

**Options:**
- `-c, --clean <bool>`    - Keep the temporary files and directories. (default: `true`)
- `-m, --managed <bool>`  - Set `"managed: false"` to allow direct editing in Kibana. (Default: `true`)

**Arguments:**
- `[input_dir]`           - A directory containing the `manifest.json` file to import. (default: `.`)

### Togo Command
```
kibob togo [input_dir]
```

Bundle up the `[input_dir]/objects/*.json` objects into a distributable NDJSON file named `${input_dir}.ndjson`

**Options:**
- `-m, --managed <bool>`  - Set `"managed: false"` to allow direct editing in Kibana. (Default: `true`)

**Arguments:**
- `[input_dir]`         - Directory containing the objects to bundle (default: `.`)
