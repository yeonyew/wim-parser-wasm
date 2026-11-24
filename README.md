# wim-parser-wasm

[![CI](https://github.com/yeonyew/wim-parser-wasm/actions/workflows/ci-build.yml/badge.svg)](https://github.com/yeonyew/wim-parser-wasm/actions/workflows/ci-build.yml)

Rust와 WebAssembly로 구현한 WIM/ESD 메타데이터 파서입니다. 브라우저의
`File` 객체를 받아 WIM 헤더와 XML 메타데이터만 읽고, 이미지 목록과 컨테이너
정보를 JavaScript 객체로 반환합니다.

파일 전체를 업로드하거나 이미지 리소스를 압축 해제하지 않습니다.

## 주요 기능

- 브라우저 `File` API와 Web Worker에서 실행 가능
- WIM 헤더 208바이트와 XML 리소스 구간만 읽음
- 여러 이미지의 DISM `/Get-ImageInfo` 수준 메타데이터 반환
- 64비트 크기·오프셋을 문자열로 보존해 JavaScript 정밀도 손실 방지
- XML 엔티티, CDATA, UTF-16LE, FILETIME, 언어 목록 지원
- Rust 단위 테스트, Node.js Wasm 테스트, Playwright 브라우저 테스트 제공

## 설치

```sh
npm install wim-parser-wasm
```

## 사용법

```js
import init, { parseWim } from "wim-parser-wasm";

const input = document.querySelector("input[type=file]");
const file = input.files[0];

await init();
const info = await parseWim(file);

console.log(info.imageCount);
console.log(info.images[0].osName);
console.log(info.images[0].buildVersion);
```

TypeScript 타입은 별도 export로 제공합니다.

```ts
import type { WimInfo } from "wim-parser-wasm/types";
```

## 로컬 데모

저장소를 clone한 뒤 Wasm 산출물을 생성합니다.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
npm run build
python3 -m http.server 8000
```

브라우저에서 <http://localhost:8000/web/>를 열고 WIM 파일을 선택합니다.
데모는 선택한 파일을 서버로 업로드하지 않습니다.

`pkg/`는 빌드 산출물이므로 저장소에는 커밋하지 않습니다. npm 패키지를
배포할 때 GitHub Actions가 `pkg/`를 생성하며, `package.json`의 `files` 설정을
통해 배포 파일에 포함합니다.

## 반환값

최상위 결과는 다음 구조를 가집니다.

```ts
interface WimInfo {
  imageCount: number;
  totalBytes: string | null;
  container: ContainerInfo;
  images: ImageInfo[];
}
```

### 이미지 정보

| 필드 | 설명 |
|---|---|
| `index` | `IMAGE/@INDEX` |
| `osName` | `NAME` |
| `displayName` | `DISPLAYNAME` |
| `description` | `DESCRIPTION` |
| `displayDescription` | `DISPLAYDESCRIPTION` |
| `imageSize` | 이미지의 `TOTALBYTES` |
| `hardLinkBytes` | `HARDLINKBYTES` |
| `edition` | `WINDOWS/EDITIONID` |
| `architecture` | `WINDOWS/ARCH` 원문 |
| `productName` | `WINDOWS/PRODUCTNAME` |
| `installationType` | `WINDOWS/INSTALLATIONTYPE` |
| `hal` | `WINDOWS/HAL` |
| `productType` | `WINDOWS/PRODUCTTYPE` |
| `productSuite` | `WINDOWS/PRODUCTSUITE` |
| `majorVersion`, `minorVersion` | Windows 버전 원문 |
| `build`, `spBuild`, `spLevel` | 빌드 관련 원문 |
| `version` | `MAJOR.MINOR.BUILD` 조합 |
| `buildVersion` | `BUILD.SPBUILD` 조합 |
| `bootable` | WIM 헤더의 `BootIndex`가 가리키는 이미지인지 여부 |
| `wimBoot` | `WIMBOOT` 원문 |
| `flags` | 이미지 `FLAGS` 원문 |
| `systemRoot` | `WINDOWS/SYSTEMROOT` |
| `languages` | 언어 목록, 기본 언어, fallback 정보 |
| `fileCount`, `directoryCount` | 파일·디렉터리 수 원문 |
| `createdTime`, `modifiedTime` | FILETIME 원문 및 해석 가능한 UTC 값 |

### 컨테이너 정보

`container`에는 다음 정보가 포함됩니다.

- `imageType`, `fileSize`, `headerSize`, `formatVersion`
- `flags`, `compressionChunkSize`, `guid`
- `partNumber`, `totalParts`, `bootIndex`
- `lookupTable`, `xml`, `bootMetadata`, `integrity` 리소스 정보
- `hasIntegrityTable`

리소스 정보의 공통 필드는 `offset`, `size`, `originalSize`, `flags`입니다.
표시되는 리소스 중 실제로 읽는 것은 헤더와 XML뿐입니다.

## 값 처리 규칙

- XML 값은 XML 규칙에 따라 디코딩하지만 공백, 대소문자, 선행 0은 보존합니다.
- 없는 필드는 `null`, 존재하지만 비어 있는 필드는 빈 문자열입니다.
- `NAME`과 `DISPLAYNAME`은 서로 대체하지 않습니다.
- XML 숫자와 64비트 값은 문자열로 반환합니다.
- 이미지 인덱스, 이미지 수, 헤더의 32비트 필드는 숫자입니다.
- `version`과 `buildVersion`은 필요한 원문 필드가 유효한 경우에만 생성됩니다.
- `bootable`은 헤더의 지정 정보만 의미하며 실제 부팅을 검증하지 않습니다.
- `createdTime`과 `modifiedTime`은 이미지 파일의 파일시스템 수정 시각이 아닙니다.

## 지원 범위와 제한

- 표준 `MSWIM` 시그니처와 208바이트 WIM 헤더
- WIM 포맷 버전 `0x10D00`, `0xE00`
- 같은 메타데이터 구조를 사용하는 standalone ESD
- XML 최대 32MiB
- XML 노드 최대 1,000,000개
- 이미지 최대 100,000개

다음은 지원하지 않습니다.

- 분할 SWM
- pipable WIM
- 압축·solid·spanned XML
- 이미지 내부 파일, 레지스트리, 업데이트, 드라이버, 앱 인벤토리 분석
- 무결성 테이블 검증

## 개발

필요한 도구는 Rust 1.88 이상, Node.js 22 이상, Python 3입니다.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked

cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked

npm run build
npm run test:wasm
```

브라우저 통합 테스트를 실행하려면 Playwright와 Chromium을 설치합니다.

```sh
npm install --no-save --no-package-lock playwright
npx playwright install --with-deps chromium
npm run test:browser
```

## GitHub Actions와 릴리즈

### CI

`.github/workflows/ci-build.yml`은 다음을 실행합니다.

- PR 및 `main` push 검증
- Rust 포맷, Clippy, 단위 테스트
- 릴리스 Wasm 빌드
- Node.js Wasm 테스트
- 브라우저 통합 테스트
- 검증된 `pkg/` artifact 생성

### npm 배포

`.github/workflows/publish.yml`은 `v*` Git 태그에서 실행됩니다. Git 태그와
`package.json`의 버전은 정확히 같아야 합니다.

| `package.json` 버전 | Git 태그 | npm dist-tag |
|---|---|---|
| `0.2.0-beta.1` | `v0.2.0-beta.1` | `beta` |
| `0.2.0-rc.1` | `v0.2.0-rc.1` | `rc` |
| `0.2.0` | `v0.2.0` | `latest` |

예를 들어 beta 릴리즈는 다음처럼 만듭니다.

```sh
npm version 0.2.0-beta.1 --no-git-tag-version
git add package.json
git commit -m "chore: prepare v0.2.0-beta.1"
git push

git tag -a v0.2.0-beta.1 -m "Release v0.2.0-beta.1"
git push origin v0.2.0-beta.1
```

릴리즈 workflow는 먼저 CI를 통과한 `pkg/`를 사용한 뒤 npm에 publish합니다.
Trusted Publisher를 사용할 때 npm 설정의 workflow filename은 `publish.yml`이어야
합니다. 현재 workflow는 `npm publish`를 직접 실행하므로 Trusted Publisher에서
직접 publish 권한을 허용해야 합니다. stage-only 정책을 사용하려면 workflow의
publish 명령을 `npm stage publish --tag "${{ steps.release.outputs.dist_tag }}"`로
바꾸고 npm에서 staged package를 승인합니다.

## 참고

- [Microsoft DISM DismImageInfo](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/dism/dismimageinfo-structure)
- [Microsoft DISM DismWimCustomizedInfo](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/dism/dismwimcustomizedinfo-structure)
- [Microsoft WIM reader](https://github.com/microsoft/go-winio/blob/main/wim/wim.go)

## License

MIT
