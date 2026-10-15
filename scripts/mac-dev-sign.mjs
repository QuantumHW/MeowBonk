import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, writeFileSync, readFileSync, chmodSync } from 'node:fs';
import { resolve } from 'node:path';
import { randomBytes } from 'node:crypto';
import { root } from './env.mjs';

if (process.platform !== 'darwin') throw new Error('本地开发签名仅用于 macOS');
const directory = resolve(root, '.cache/dev-signing');
mkdirSync(directory, { recursive: true, mode: 0o700 });
chmodSync(directory, 0o700);
const file = name => resolve(directory, name);
const run = (command, args) => {
  try { return execFileSync(command, args, { encoding: 'utf8', env: {...process.env, MEOWBONK_SIGN_PASSWORD: password}, stdio: ['ignore','pipe','pipe'] }); }
  catch (error) { console.error(String(error.stderr ?? '').replaceAll(password, '[hidden]')); throw new Error(`${command} 执行失败；签名未完成。请检查项目专用钥匙串与系统认证窗口。`); }
};
if (!existsSync(file('password'))) writeFileSync(file('password'), randomBytes(32).toString('hex'), {mode:0o600});
const password = readFileSync(file('password'),'utf8');
const keychain = file('development.keychain-db');
if (!existsSync(file('certificate.pem'))) {
  writeFileSync(file('openssl.cnf'), `[req]\ndistinguished_name=subject\nx509_extensions=signing\nprompt=no\n[subject]\nCN=MeowBonk Local Development\nO=MeowBonk Local Development\n[signing]\nbasicConstraints=critical,CA:false\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=critical,codeSigning\nsubjectKeyIdentifier=hash\n`, {mode:0o600});
  run('/usr/bin/openssl',['req','-new','-x509','-newkey','rsa:2048','-sha256','-days','3650','-config',file('openssl.cnf'),'-keyout',file('private-key.pem'),'-out',file('certificate.pem'),'-passout','env:MEOWBONK_SIGN_PASSWORD']);
  chmodSync(file('private-key.pem'),0o600);
}
if (!existsSync(keychain)) {
  const original = run('/usr/bin/security',['list-keychains','-d','user']);
  const searchList = [...original.matchAll(/"([^"\n]+)"/g)].map(match=>match[1]);
  try { run('/usr/bin/security',['create-keychain','-p',password,keychain]); }
  finally { run('/usr/bin/security',['list-keychains','-d','user','-s',...searchList]); }
}
run('/usr/bin/security',['unlock-keychain','-p',password,keychain]);
if (!existsSync(file('imported'))) {
  run('/usr/bin/openssl',['pkcs12','-export','-inkey',file('private-key.pem'),'-in',file('certificate.pem'),'-out',file('identity.p12'),'-passin',`file:${file('password')}`,'-passout','env:MEOWBONK_SIGN_PASSWORD']);
  chmodSync(file('identity.p12'),0o600);
  run('/usr/bin/security',['import',file('identity.p12'),'-k',keychain,'-P',password,'-T','/usr/bin/codesign']);
  // Access is restricted to Apple's signing tools in this project-specific keychain.
  run('/usr/bin/security',['set-key-partition-list','-S','apple-tool:,apple:','-s','-k',password,keychain]);
  writeFileSync(file('imported'),'1',{mode:0o600});
}
const fingerprint = run('/usr/bin/openssl',['x509','-in',file('certificate.pem'),'-noout','-fingerprint','-sha1']).split('=')[1].trim().replaceAll(':','');
const bundle = resolve(root,'src-tauri/target/debug/bundle/macos/MeowBonk.app');
const requirement = `designated => identifier "com.meowbonk.desktop" and certificate leaf = H"${fingerprint}"`;
run('/usr/bin/codesign',['--force','--sign',fingerprint,'--keychain',keychain,'--identifier','com.meowbonk.desktop','--requirements',`=${requirement}`,'--timestamp=none',bundle]);
run('/usr/bin/codesign',['--verify','--strict','--verbose=2',bundle]);
console.log('macOS 本地开发签名已验证：com.meowbonk.desktop（项目专用证书）');
