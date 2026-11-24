// Run after generating pkg/: node tests/wasm.mjs
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { File } from 'node:buffer';
import init, { parseWim } from '../pkg/wim_info_wasm.js';

await init({ module_or_path: await readFile(new URL('../pkg/wim_info_wasm_bg.wasm', import.meta.url)) });

function fixture(xml, offset = 208) {
  const payload = Buffer.from(xml, 'utf16le');
  const header = Buffer.alloc(208);
  header.write('MSWIM\0\0\0', 0, 'binary');
  header.writeUInt32LE(208, 8);
  header.writeUInt32LE(0x10d00, 12);
  header.writeUInt16LE(1, 40);
  header.writeUInt16LE(1, 42);
  header.writeUInt32LE(1, 44);
  header.writeBigUInt64LE(BigInt(payload.length), 72);
  header.writeBigUInt64LE(BigInt(offset), 80);
  header.writeBigUInt64LE(BigInt(payload.length), 88);
  return { header, payload, parts: [header, Buffer.alloc(offset - 208), payload] };
}

const calls = [];
class TrackedFile extends File {
  slice(start, end, type) {
    calls.push([start, end]);
    return super.slice(start, end, type);
  }
  arrayBuffer() { throw new Error('Whole-file reads are forbidden'); }
}
const xml = '\uFEFF<WIM><IMAGE INDEX="1"><NAME>Windows &amp; 테스트</NAME><WINDOWS><ARCH>9</ARCH><EDITIONID>Professional</EDITIONID><VERSION><BUILD>26100</BUILD><SPBUILD>1111</SPBUILD></VERSION></WINDOWS></IMAGE></WIM>';
const f = fixture(xml, 6 * 1024 * 1024);
const file = new TrackedFile(f.parts, 'synthetic.wim');
const result = await parseWim(file);
assert.deepEqual({ imageCount: result.imageCount, images: result.images.map(({index, osName, displayName, edition, architecture, buildVersion, build, spBuild}) => ({index, osName, displayName, edition, architecture, buildVersion, build, spBuild})) }, {
  imageCount: 1,
  images: [{ index: 1, osName: 'Windows & 테스트', displayName: null,
    edition: 'Professional', architecture: '9', buildVersion: '26100.1111',
    build: '26100', spBuild: '1111' }],
});
assert.deepEqual(calls, [[0, 208], [6 * 1024 * 1024, file.size]]);
await assert.rejects(parseWim(new File([Buffer.alloc(208)], 'invalid.wim')), /Invalid WIM signature/);
await assert.rejects(parseWim(new File([], 'empty.wim')), /Invalid or unsupported file size/);
const empty = fixture('<WIM><IMAGE INDEX="1"><NAME/></IMAGE></WIM>');
const emptyResult = await parseWim(new File(empty.parts, 'empty-fields.wim'));
assert.equal(emptyResult.images[0].osName, '');
assert.equal(emptyResult.images[0].buildVersion, null);
console.log('PASS: Wasm/File API, exact header/XML reads, raw text, explicit null, error propagation');
const detailed = fixture('<WIM><TOTALBYTES>9007199254740993</TOTALBYTES><IMAGE INDEX="1"><NAME>Pro</NAME><DESCRIPTION>설명</DESCRIPTION><TOTALBYTES>9007199254740995</TOTALBYTES><FILECOUNT>123</FILECOUNT><DIRCOUNT>45</DIRCOUNT><CREATIONTIME><HIGHPART>0x019DB1DE</HIGHPART><LOWPART>0xD53E8000</LOWPART></CREATIONTIME><WINDOWS><ARCH>9</ARCH><INSTALLATIONTYPE>Client</INSTALLATIONTYPE><VERSION><MAJOR>10</MAJOR><MINOR>0</MINOR><BUILD>26100</BUILD><SPBUILD>1111</SPBUILD><SPLEVEL>0</SPLEVEL></VERSION><LANGUAGES><LANGUAGE>ko-KR</LANGUAGE><LANGUAGE>en-US</LANGUAGE><DEFAULT>ko-KR</DEFAULT></LANGUAGES></WINDOWS></IMAGE></WIM>');
detailed.header.writeUInt32LE(1, 120);
const detail = await parseWim(new File(detailed.parts, 'details.wim'));
assert.equal(detail.totalBytes, '9007199254740993');
assert.equal(detail.container.bootIndex, 1);
assert.equal(detail.container.xml.offset, '208');
assert.equal(detail.container.imageType, 'WIM');
assert.equal(detail.images[0].description, '설명');
assert.equal(detail.images[0].imageSize, '9007199254740995');
assert.equal(detail.images[0].version, '10.0.26100');
assert.equal(detail.images[0].bootable, true);
assert.equal(detail.images[0].fileCount, '123');
assert.equal(detail.images[0].directoryCount, '45');
assert.equal(detail.images[0].createdTime.utc, '1970-01-01T00:00:00Z');
assert.equal(detail.images[0].modifiedTime, null);
assert.deepEqual(detail.images[0].languages, { values: ['ko-KR', 'en-US'], count: 2, default: 'ko-KR', defaultIndex: 0, fallbacks: [] });
console.log('PASS: DISM details, languages, timestamps, boot designation, 64-bit values');
