import { spawn, spawnSync } from 'node:child_process';
import { root, env } from './env.mjs';
const child = spawn(process.platform === 'win32' ? 'tauri.cmd' : 'tauri', process.argv.slice(2), { cwd: root, env, stdio: 'inherit', shell: process.platform === 'win32' });
child.on('error', error => { console.error(error.message); process.exitCode = 1; });
child.on('exit', code => {
  process.exitCode = code ?? 1;
  const args = process.argv.slice(2);
  if (code === 0 && process.platform === 'darwin' && args[0] === 'build' && args.includes('--debug') && (args.includes('app') || args.includes('--bundles=app'))) {
    const signing = spawnSync(process.execPath, ['scripts/mac-dev-sign.mjs'], {cwd:root,env,stdio:'inherit'});
    process.exitCode = signing.status ?? 1;
  }
});
