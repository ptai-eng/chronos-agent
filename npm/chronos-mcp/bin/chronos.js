#!/usr/bin/env node
"use strict";

const { runBinary } = require("../lib/launcher");

runBinary("chronos", process.argv.slice(2)).catch((err) => {
  console.error(`[chronos] ${err.message}`);
  process.exit(1);
});
