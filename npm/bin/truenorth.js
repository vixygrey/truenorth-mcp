#!/usr/bin/env node
// Thin launcher: resolve the platform-native TrueNorth-MCP binary and spawn it with stdio
// passthrough, or scaffold the specs/ cockpit on `init`. The logic lives in lib/runner.js
// so it stays testable. Design: Part II §7.

'use strict';

const { run, realDeps } = require('../lib/runner.js');

run(realDeps());
