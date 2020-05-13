const { createLogger, format, transports } = require('winston');
const path = require('path');

const label = (label) =>
  createLogger({
    level: 'info',
    format: format.combine(
      format.colorize(),
      format.label({ label }),
      format.timestamp({ format: 'YYYY-MM-DD HH:mm:ss.SSS' }),
      format.printf(
        (info) =>
          `${info.timestamp} [${info.level}] ` +
          '\033[35m' +
          info.label +
          '\033[39m: ' +
          info.message
      )
    ),
    transports: [new transports.Console()],
  });

module.exports = { label };
