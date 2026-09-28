'use strict';

const assert = require('node:assert/strict');
const { add } = require('./src/add.js');

assert.equal(add(2, 3), 5);
