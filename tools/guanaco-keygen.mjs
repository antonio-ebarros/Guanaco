#!/usr/bin/env node
/**
 * Gerador Oficial de Chaves de Licença Offline do Guanaco Pro (Ed25519)
 * Criado por Antonio E. Barros
 *
 * Uso:
 *   node tools/guanaco-keygen.mjs --init
 *   node tools/guanaco-keygen.mjs --holder "Nome do Cliente" --email "cliente@email.com"
 */
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');
const KEYS_DIR = path.join(ROOT, '.keys');
const PRIV_PATH = path.join(KEYS_DIR, 'guanaco_pro_private.pem');
const PUB_PATH = path.join(KEYS_DIR, 'guanaco_pro_public.json');

function toBase64Url(buf) {
  return Buffer.from(buf)
    .toString('base64')
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/g, '');
}

function ensureKeyPair() {
  if (!fs.existsSync(KEYS_DIR)) fs.mkdirSync(KEYS_DIR, { recursive: true });
  if (fs.existsSync(PRIV_PATH) && fs.existsSync(PUB_PATH)) {
    const privPem = fs.readFileSync(PRIV_PATH, 'utf8');
    const pubInfo = JSON.parse(fs.readFileSync(PUB_PATH, 'utf8'));
    return { privPem, pubInfo };
  }
  const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
  const privPem = privateKey.export({ type: 'pkcs8', format: 'pem' });
  const spkiDer = publicKey.export({ type: 'spki', format: 'der' });
  // Em SPKI Ed25519 (RFC 8410), os últimos 32 bytes são a chave pública Ed25519 bruta
  const rawPub = spkiDer.subarray(spkiDer.length - 32);
  const pubInfo = {
    algorithm: 'Ed25519',
    createdAt: new Date().toISOString(),
    publicKeyHex: rawPub.toString('hex'),
    publicKeyRustBytes: `[${[...rawPub].join(', ')}]`,
  };
  fs.writeFileSync(PRIV_PATH, privPem, { mode: 0o600 });
  fs.writeFileSync(PUB_PATH, JSON.stringify(pubInfo, null, 2));
  return { privPem, pubInfo };
}

function generateLicense({ holder, email, tier = 'pro' }) {
  const { privPem, pubInfo } = ensureKeyPair();
  const payloadObj = {
    holder: holder || 'Guanaco Pro User',
    email: email || '',
    tier,
    issuedAt: new Date().toISOString().slice(0, 10),
  };
  const payloadB64 = toBase64Url(JSON.stringify(payloadObj));
  const sig = crypto.sign(null, Buffer.from(payloadB64, 'utf8'), privPem);
  const sigB64 = toBase64Url(sig);
  return {
    key: `GUANACO-PRO-${payloadB64}.${sigB64}`,
    payload: payloadObj,
    pubInfo,
  };
}

const args = process.argv.slice(2);
const getArg = flag => {
  const idx = args.indexOf(flag);
  return idx >= 0 && args[idx + 1] ? args[idx + 1] : null;
};

const holder = getArg('--holder') || 'Antonio E. Barros';
const email = getArg('--email') || 'founder@guanaco.local';
const res = generateLicense({ holder, email });

console.log('=== GUANACO PRO KEYGEN (Ed25519 Offline) ===');
console.log('Chave Pública (Rust [u8; 32]):', res.pubInfo.publicKeyRustBytes);
console.log('Titular:', res.payload.holder, res.payload.email ? `<${res.payload.email}>` : '');
console.log('Data:', res.payload.issuedAt);
console.log('\nCHAVE DE LICENÇA GERADA:\n');
console.log(res.key);
console.log('');
