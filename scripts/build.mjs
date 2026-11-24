import { spawnSync } from 'node:child_process';
import { homedir } from 'node:os';
import { delimiter, join } from 'node:path';

const cargoHome = process.env.CARGO_HOME || join(homedir(), '.cargo');
const pathKey =
  Object.keys(process.env).find((key) => key.toUpperCase() === 'PATH') ||
  'PATH';
const env = {
  ...process.env,
  [pathKey]: [process.env[pathKey], join(cargoHome, 'bin')]
    .filter(Boolean)
    .join(delimiter),
};

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: new URL('../', import.meta.url),
    env,
    stdio: 'inherit',
  });

  if (result.error) {
    console.error(`Failed to run ${command}: ${result.error.message}`);
    process.exit(1);
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

run('cargo', [
  'build',
  '--locked',
  '--release',
  '--target',
  'wasm32-unknown-unknown',
  '--target-dir',
  'target',
]);
run('wasm-bindgen', [
  '--target',
  'web',
  '--out-dir',
  'pkg',
  'target/wasm32-unknown-unknown/release/wim_info_wasm.wasm',
]);
