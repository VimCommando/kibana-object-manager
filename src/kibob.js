#!/usr/bin/env node
const R = require('ramda');
const fs = require('fs').promises;
const fetch = require('node-fetch');
const FormData = require('form-data');
const path = require('path');
const yargs = require('yargs');

const logger = require('./logger.js').label('kibob');

// Configures command-line arguments
const argv = yargs
  .command(
    'export',
    'Export saved obejcts from Kibana',
    {
      url: {
        alias: 'u',
        description:
          'The connection URL for the Kibana instance, must include protocol,' +
          'may include username and password',
        default: 'http://localhost:5601',
      },
      file: {
        alias: 'f',
        description: 'Name of file to export to',
        type: 'string',
        default: 'saved_objects.ndjson',
      },
      search: {
        alias: 's',
        description: 'Search term to find objects by',
      },
      types: {
        alias: 't',
        description: 'Array of types to export',
        type: 'string',
        array: true,
        default: ['index-pattern','visualization','lens','dashboard'],
      }
    },
    async (argv) => {
      setLogger(argv);
      saveObjects(argv.file, await findObjects(argv));
    }
  )
  .command(
    'import',
    'Import saved objects into Kibana',
    {
      //dir: {
      //  alias: 'd',
      //  description: 'Target directory, will automatically bundle .json files',
      //  type: 'string',
      //},
      file: {
        alias: 'f',
        description: 'Single saved_objects.ndjson file to import',
        type: 'string',
        default: 'saved_objects.ndjson',
      },
      overwrite: {
        alias: 'o',
        description:
          'forces overwrite of existing objects',
        type: 'boolean',
        default: false,
      },
      url: {
        alias: 'u',
        description: 'The connection URL for the Kibana server',
        type: 'string',
        default: 'http://localhost:5601',
      },
    },
    (argv) => {
      setLogger(argv);
      importObjects(argv)
    }
  )
  .command(
    'bundle',
    'Bundles multiple .json files into one .ndjson file',
    {
      dir: {
        alias: 'd',
        description: 'Target input directory',
        type: 'string',
      },
      file: {
        alias: 'f',
        description: 'Output filename',
        type: 'string',
        default: 'saved_objects.ndjson',
      },
    },
    (argv) => {
      setLogger(argv);
      bundleObjects(argv);
    }
  )
  .command(
    'unbundle',
    'Unbundle single .ndjson file into multiple .json files',
    {
      dir: {
        alias: 'd',
        description: 'Target output directory',
        type: 'string',
        default: 'saved_objects',
      },
      file: {
        alias: 'f',
        description: 'Input filename',
        type: 'string',
        default: 'saved_objects.ndjson',
      },
    },
    (argv) => {
      setLogger(argv);
      unbundleObjects(argv);
    }
  )
  .option('test', {
    description: 'Test mode, only print to console',
    type: 'boolean',
  })
  .option('debug', {
    description: 'Log in debug mode',
    type: 'boolean',
  })
  .option('verbose', {
    alias: 'v',
    description: 'Log in verbose mode',
    type: 'boolean',
  })
  .help()
  .alias('help', 'h').argv;

// adjust logger level if command-line arguments were given
function setLogger(argv) {
  logger.level = argv.debug ? 'debug' : argv.verbose ? 'verbose' : 'info';
}

/* Loads saved objects from a saved_objects.ndjson file and calls the Kibana
 * create saved objects API to import them.
 * https://www.elastic.co/guide/en/kibana/master/saved-objects-api-import.html
 */

async function importObjects(argv) {
  const url = new URL(argv.url);
  url.search = argv.overwrite ? '?overwrite=true' : '';
  url.pathname = `/api/saved_objects/_import`;

  const buffer = await fs.readFile(argv.file, 'binary');
  const form = new FormData();
  form.append('file', buffer, {
    contentType: 'text/plain',
    name: 'file',
    filename: argv.file,
  });

  logger.info('loading saved objects from ' + argv.file);
  const options = {
    method: 'POST',
    headers: {
      'kbn-xsrf': true,
      form: form.getHeaders(),
    },
  };

  try {
    const res = await fetch(url, { ...options, body: form });
    const body = JSON.stringify(await res.json(), null, 2);
    if (res.status === 200) {
      logger.info(`${res.status} ${res.statusText} Response:\n${body}`);
    } else {
      logger.error(`${res.status} ${res.statusText} Error: ${body}`);
    }
  } catch (FetchError) {
    logger.error(`${FetchError.message}`);
  }
}

// Export Kibana saved objects with the find API
// https://www.elastic.co/guide/en/kibana/current/saved-objects-api-find.html
async function findObjects(argv) {
  let body = {};
  const url = new URL(argv.url);
  url.pathname = '/api/saved_objects/_find';
  url.search = '?per_page=1000';
  for (const type of argv.types)
    url.search += `&type=${type}`
  url.search
  url.search += argv.search ? `&search=${argv.search}` : '';

  logger.verbose('url.search: ' + url.search);

  const options = {
    method: 'GET',
    headers: {
      'kbn-xsrf': true,
    },
  };

  try {
    const res = await fetch(url, { ...options });
    body = await res.json();
    if (res.status === 200) {
      logger.info(`${res.status} ${res.statusText} Found: ${body.saved_objects.length} objects`);
    } else {
      logger.error(`${res.status} ${res.statusText} Error: ${body}`);
    }
  } catch (FetchError) {
    logger.error(`${FetchError.message}`);
  }
  return body.saved_objects;
}

// Write an array of JSON objects into an .ndjson file
async function saveObjects(filename, saved_objects) {
  let text = '';
  saved_objects.forEach(obj => {
    text += JSON.stringify(obj) + '\n';
  })
  try {
    const data = new Uint8Array(Buffer.from(text));
    await fs.writeFile(filename, data);

    logger.info(`Saved ${saved_objects.length} objects to ${filename}`);
  } catch (err) {
    logger.error(`${filename}: [${err.name}] ${err.message}`);
    throw err;
  }
}

// Export Kibana saved objects from the _export API
// https://www.elastic.co/guide/en/kibana/current/saved-objects-api-export.html
async function exportObjects(argv) {
  const url = new URL(argv.url);
  url.pathname = '/api/saved_objects/_export';

  const options = {
    method: 'GET',
    headers: {
      'kbn-xsrf': true,
    },
  };

  try {
    const res = await fetch(url, { ...options });
    const body = JSON.stringify(await res.json(), null, 2);
    if (res.status === 200) {
      logger.info(`${res.status} ${res.statusText} Response:\n${body}`);
    } else {
      logger.error(`${res.status} ${res.statusText} Error: ${body}`);
    }
  } catch (FetchError) {
    logger.error(`${FetchError.message}`);
  }
}

// Convert .ndjson file into separate .json files
async function unbundleObjects(argv) {
  const path = argv.dir;
  try {
    const buffer = await fs.readFile(argv.file, 'binary');
    await fs.mkdir(path);
    
    buffer.split('\n').forEach(async (obj) => {
      try {
        const json = obj && JSON.parse(obj);
        if(json.type) {
          const filename = `${json.attributes.title}.${json.type}.json`;
          logger.debug(filename);
          const data = new Uint8Array(Buffer.from(JSON.stringify(json, null, 2)));
          await fs.writeFile(`${path}/${filename}`, data);
        }
      } catch (SyntaxError) {
        console.log(SyntaxError)
        logger.debug(`Failed to parse: ${SyntaxError}`);
      }
    });
  } catch (err) {
    logger.error(err);
  }
}

// Convert directory of .json files into single .ndjson
async function bundleObjects(argv) {
  let output = '';
  let i = 0;
  try {
    const dir = await fs.opendir(argv.dir);
    for await (const dirent of dir) {
      const buffer = await fs.readFile(`${argv.dir}/${dirent.name}`, 'binary');
      logger.debug(`Bundling '${argv.dir}/${dirent.name}'`);
      output += JSON.stringify(JSON.parse(buffer)) + '\n';
      i++;
    }
    const data = new Uint8Array(Buffer.from(output));
    await fs.writeFile(argv.file, data);
    logger.info(`Wrote ${i} objects to ${argv.file}`);
  } catch (err) {
    logger.error(err);
  }
}
