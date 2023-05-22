> ⚠️ NOTICE ⚠️ \
> This project is deprecated. All of the useful JSON processing is now handled by [`json-remix`](https://github.com/VimCommando/json-remix). So you can pull the saved objects from Kibana (either the UI or API) and post-process them with `json-remix` instead.

# Kibana Object Manager

A small utility to import and export Kibana saved objects through the Kibana [import object API](https://www.elastic.co/guide/en/kibana/current/saved-objects-api-import.html) and [export objects API](https://www.elastic.co/guide/en/kibana/current/saved-objects-api-export.html)

These objects can be unbundled into individual `.json` files for easy version tracking in a git repository.

Or bundled from multiple files into a single `.ndjson` for easy distribution or import.

## Installation

1. Clone this repository to your local machine
2. Run `npm install` from the `/src` directory
3. Symlink the `kibob.js` file into your executable path:

```
$ ln -s ~/github/kibana-object-manager/src/kibob.js ~/bin/kibob
```

## Usage

### Import saved objects into Kibana

Take the `saved_objects.ndjson` file and import it through Kibana's [saved objects API](https://www.elastic.co/guide/en/kibana/master/saved-objects-api-import.html)

```
kibob import -u <kibana_url> -f <saved_objects.ndjson>
```

**Options**

- `-f | --file` - filename to load
- `-o | --overwrite` - clobber any existing saved objects.
- `-u | --url` - Kibana URL, default: `http://localhost:5601`

### Export saved objects from Kibana

Use Kibana's [find API](https://www.elastic.co/guide/en/kibana/current/saved-objects-api-find.html) to search for objects to export.

The saved object will strip the `updated_at` and `version` fields; as this causes conflicts with your source control versioning.

```
kibob export -u <kibana_url> -s <search_term>
```

**Options**

- `-s | --search` - Query term to filter objects (tip: prefix your objects!)
- `-t | --types` - Array of object types to export, default: `index-pattern visualization lens dashboard`
- `-f | --file` - filename to write to, default: `saved_objects.ndjson`
- `-u | --url` - Kibana URL, default: `http://localhost:5601`

### Bundle directory of files into a single .ndjson file

Read in a directory full of `.json` files and bundle it into a single `.ndjson` file.

```
kibob bundle -d <dir> -f <bundle.ndjson>
```

**Options**

- `-d | --dir` - Directory to bundle into a single file
- `-f | --file` - filename to write to, default: `saved_objects.ndjson`

### Unbundle saved objects into individual files

Take the single `.ndjson` file and split it into pretty-printed `.json` files.

```
kibob unbundle -f <saved_objects.ndjson> -d <dir>
```

Output files will be named:

```
${dir}/${object.title}.${object.type}.json
```

**Options**

- `-d | --dir` - Directory to write individual `.json` files to
- `-f | --file` - filename to read from, default: `saved_objects.ndjson`

## Compatibility

This has only been tested against Kibana 7.6
