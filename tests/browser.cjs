// Requires: npm install --no-save playwright && npx playwright install chromium
// Run after generating pkg/: node tests/browser.cjs
const { chromium } = require('playwright');
const { spawn } = require('node:child_process');
const assert = require('node:assert/strict');
const path = require('node:path');
const root = path.resolve(__dirname, '..');
const port = 18765;
const origin = `http://127.0.0.1:${port}`;
function fixture(xml, offset = 208) {
  const payload = Buffer.from(xml, 'utf16le');
  const file = Buffer.alloc(offset + payload.length);
  file.write('MSWIM\0\0\0', 0, 'binary');
  file.writeUInt32LE(208, 8);
  file.writeUInt32LE(0x10d00, 12);
  file.writeUInt16LE(1, 40);
  file.writeUInt16LE(1, 42);
  file.writeUInt32LE(1, 44);
  file.writeBigUInt64LE(BigInt(payload.length), 72);
  file.writeBigUInt64LE(BigInt(offset), 80);
  file.writeBigUInt64LE(BigInt(payload.length), 88);
  payload.copy(file, offset);
  return file;
}
(async () => {
  const server = spawn('python3', ['-m', 'http.server', String(port), '--bind', '127.0.0.1'], { cwd: root, stdio: 'ignore' });
  let browser;
  try {
    let available = false;
    for (let i = 0; i < 100; i++) {
      try { available = (await fetch(`${origin}/web/`)).ok; } catch {}
      if (available) break;
      await new Promise(resolve => setTimeout(resolve, 50));
    }
    assert(available, 'HTTP server did not start');
    browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
    const page = await browser.newPage();
    await page.goto(`${origin}/web/`);
    const xml = '<WIM><IMAGE INDEX="1"><NAME>Windows 11 Pro</NAME><WINDOWS><EDITIONID>Professional</EDITIONID><ARCH>9</ARCH><VERSION><BUILD>26100</BUILD><SPBUILD>1111</SPBUILD></VERSION></WINDOWS></IMAGE></WIM>';
    const bytes = fixture(xml, 6 * 1024 * 1024);
    await page.locator('#file').setInputFiles({ name: 'synthetic.wim', mimeType: 'application/octet-stream', buffer: bytes });
    await page.waitForFunction(() => document.querySelector('#result').textContent.startsWith('{'));
    const result = JSON.parse(await page.locator('#result').textContent());
    assert.equal(result.imageCount, 1);
    assert.equal(result.images[0].osName, 'Windows 11 Pro');
    assert.equal(result.images[0].edition, 'Professional');
    assert.equal(result.images[0].architecture, '9');
    assert.equal(result.images[0].buildVersion, '26100.1111');
    assert.equal(result.images[0].displayName, null);
    // Instrument File reads to verify the Wasm API reads only header and XML.
    const reads = await page.evaluate(async ({ raw, xmlBytes }) => {
      const { default: init, parseWim } = await import('/pkg/wim_info_wasm.js');
      await init();
      const calls = [];
      class TrackedFile extends File {
        slice(start, end, type) { calls.push([start, end]); return super.slice(start, end, type); }
        arrayBuffer() { throw new Error('Whole-file reads are forbidden'); }
      }
      const file = new TrackedFile([Uint8Array.from(raw)], 'test.wim');
      await parseWim(file);
      return calls;
    }, { raw: [...bytes], xmlBytes: Buffer.byteLength(xml, 'utf16le') });
    assert.deepEqual(reads, [[0, 208], [6 * 1024 * 1024, bytes.length]]);
    await page.locator('#file').setInputFiles({ name: 'invalid.wim', mimeType: 'application/octet-stream', buffer: Buffer.alloc(208) });
    await page.waitForFunction(() => document.querySelector('#result').textContent.startsWith('오류:'));
    assert.match(await page.locator('#result').textContent(), /Invalid WIM signature/);
    console.log('PASS: Worker/File/Wasm result, explicit null, exact range reads, invalid input');
  } finally {
    if (browser) await browser.close();
    server.kill();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
