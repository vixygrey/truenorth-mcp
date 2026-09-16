#!/usr/bin/env node
// Thin launcher: resolve the platform-native TrueNorth-MCP binary and spawn it with stdio
// passthrough. The logic lives in lib/runner.js so it stays testable. Scaffolding is the
// runtime's truenorth_scaffold_project tool, not the wrapper (#182). Design: Part II §7.

'use strict';

const { run, realDeps } = require('../lib/runner.js');

run(realDeps());
