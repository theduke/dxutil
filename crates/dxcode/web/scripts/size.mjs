import {statSync} from 'node:fs';
const bytes = statSync('dist/code.iife.js').size;
if (bytes > 1_200_000) throw new Error(`Code bundle exceeds budget: ${bytes} bytes`);
console.log(`Code bundle: ${bytes} / 1200000 bytes`);
