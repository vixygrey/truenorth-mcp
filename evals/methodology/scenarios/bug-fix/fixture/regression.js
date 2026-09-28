'use strict';

const assert = require('node:assert/strict');
const { normalize } = require('./src/normalize.js');

assert.equal(normalize('  TRUE NORTH  '), 'true north');
