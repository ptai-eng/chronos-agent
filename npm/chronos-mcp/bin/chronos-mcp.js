#!/usr/bin/env node
"use strict";

const { runBinary } = require("../lib/launcher");

runBinary("chronos-mcp", process.argv.slice(2)).catch((err) => {
  console.error(`[chronos-mcp] ${err.message}`);
  process.exit(1);
});
