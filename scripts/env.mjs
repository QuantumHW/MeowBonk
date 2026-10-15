import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
export const root = fileURLToPath(new URL('../', import.meta.url));
export const env = { ...process.env, CARGO_HOME: resolve(root, '.cache/cargo'), CARGO_TARGET_DIR: resolve(root, 'src-tauri/target') };
