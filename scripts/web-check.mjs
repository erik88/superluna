// Run the web-check module in Node's WebAssembly engine: the same engine family browsers use.
import { readFileSync } from 'node:fs';

const path = process.argv[2] ?? 'target/wasm32-unknown-unknown/release/ns_web_check.wasm';
const { instance } = await WebAssembly.instantiate(readFileSync(path), {});
const { passing_replays, total_replays } = instance.exports;
const pass = passing_replays();
const total = total_replays();
console.log(`web-check: ${pass}/${total} replays match desktop checkpoints under WebAssembly`);
process.exit(pass === total ? 0 : 1);
