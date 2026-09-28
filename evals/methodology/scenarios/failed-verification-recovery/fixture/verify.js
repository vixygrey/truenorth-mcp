'use strict';

const fs = require('node:fs');

if (fs.readFileSync('status.txt', 'utf8').trim() !== 'fixed') {
  console.error('status is not fixed');
  process.exit(1);
}
