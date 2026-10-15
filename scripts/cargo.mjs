import { spawn } from 'node:child_process';
import { resolve } from 'node:path';
import { root, env } from './env.mjs';
const child = spawn('cargo', process.argv.slice(2), { cwd: resolve(root, 'src-tauri'), env, stdio: 'inherit' });
child.on('error', error => { console.error(error.message); process.exitCode = 1; });
child.on('exit', code => { process.exitCode = code ?? 1; });
