# WIM 상세 정보 파서 — Rust / Wasm

브라우저의 File을 Worker로 전달하면 Rust가 헤더 208바이트와 XML 구간만 읽습니다.
DISM `/Get-ImageInfo /ImageFile:... /Index:...`에 대응하는 이미지 메타데이터와
WIM 컨테이너 정보를 반환합니다. `web/index.html`에서는 전체 항목을 표로 표시합니다.

## 바로 실행

빌드 결과물인 `pkg/`를 소스와 함께 Git에 포함합니다.
저장소 루트에서 `pnpm install`을 실행하면 별도 Rust 빌드 없이 사용할 수 있습니다.
Rust 소스를 변경한 경우 아래 **소스 빌드** 절차로 `pkg/`를 갱신하세요.

빌드 후 이 패키지 디렉터리에서 다음을 실행하세요.

```sh
python3 -m http.server 8000
```

http://localhost:8000/web/ 에 접속해 WIM 파일을 선택합니다.
선택한 파일은 네트워크로 업로드하지 않습니다.

직접 호출하는 경우:

```js
import init, { parseWim } from '@zeroup/wim-info-wasm';

await init();
const result = await parseWim(file);
```

`@zeroup/core`는 `workspace:*` 의존성으로 이 패키지를 사용합니다.
반환값 타입은 `@zeroup/wim-info-wasm/types`에서 가져올 수 있습니다.

```ts
import type { WimInfo } from '@zeroup/wim-info-wasm/types';
```

위 패키지 import는 Vite 등 번들러에서 사용합니다. `web/` 데모는
생성된 `pkg/wim_parser_wasm.js`를 상대 경로로 불러옵니다.

## 반환 항목

XML 경로는 별도 표기가 없으면 `WIM/IMAGE` 기준입니다.

| 결과 필드 | 의미 / 원본 위치 |
|---|---|
| imageCount | 헤더 이미지 수. XML 이미지 개수와 일치 여부 검사 |
| totalBytes | WIM/TOTALBYTES 원문 |
| images[].index | IMAGE/@INDEX, 숫자 |
| osName | NAME |
| displayName | DISPLAYNAME |
| description | DESCRIPTION |
| displayDescription | DISPLAYDESCRIPTION |
| imageSize | TOTALBYTES. 선택한 WIM 파일 자체의 크기와 구분 |
| hardLinkBytes | HARDLINKBYTES |
| architecture | WINDOWS/ARCH. `"9"` 같은 원문 코드를 보존 |
| edition | WINDOWS/EDITIONID |
| productName | WINDOWS/PRODUCTNAME |
| installationType | WINDOWS/INSTALLATIONTYPE |
| hal | WINDOWS/HAL |
| productType | WINDOWS/PRODUCTTYPE |
| productSuite | WINDOWS/PRODUCTSUITE |
| majorVersion | WINDOWS/VERSION/MAJOR |
| minorVersion | WINDOWS/VERSION/MINOR |
| build | WINDOWS/VERSION/BUILD |
| spBuild | WINDOWS/VERSION/SPBUILD |
| spLevel | WINDOWS/VERSION/SPLEVEL |
| version | MAJOR.MINOR.BUILD 조합, 예: `10.0.26100` |
| buildVersion | BUILD.SPBUILD 조합, 예: `26100.1111` |
| bootable | 헤더 BootIndex가 이 이미지 인덱스를 가리키는지 여부 |
| wimBoot | WIMBOOT 원문. bootable과 별개인 레거시 WIMBoot 속성 |
| flags | FLAGS 원문 |
| systemRoot | WINDOWS/SYSTEMROOT |
| languages.values | WINDOWS/LANGUAGES/LANGUAGE 목록 |
| languages.count | 언어 목록 길이 |
| languages.default | WINDOWS/LANGUAGES/DEFAULT |
| languages.defaultIndex | 기본 언어가 목록에 있는 경우 0부터 시작하는 인덱스 |
| languages.fallbacks | FALLBACK의 LANGUAGE 속성과 텍스트 |
| fileCount | FILECOUNT |
| directoryCount | DIRCOUNT |
| createdTime | CREATIONTIME의 highPart / lowPart 원문과 utc 파생값 |
| modifiedTime | LASTMODIFICATIONTIME의 highPart / lowPart 원문과 utc 파생값 |

`DismImageInfo.CustomizedInfo`에 해당하는 파일 수·디렉터리 수·시각은
이미지 객체 안에 함께 제공합니다. 구조체의 메모리 크기나 포인터 값은 이미지 정보가
아니므로 반환하지 않습니다. `languages.count/defaultIndex`가 API의
LanguageCount/DefaultLanguageIndex에 대응합니다.

| container 필드 | 의미 |
|---|---|
| imageType | WIM 컨테이너임을 나타내는 `WIM` |
| fileSize | 선택한 파일 자체의 크기, bytes |
| headerSize | 헤더 크기 |
| formatVersion | WIM 포맷 버전 정수 |
| flags | 원래 헤더 플래그 비트 |
| compressionChunkSize | 헤더의 압축 청크 크기 |
| guid | WIM GUID |
| partNumber / totalParts | 분할 번호 / 전체 분할 수 |
| bootIndex | 부팅 지정 인덱스. 0이면 미지정 |
| lookupTable | 리소스 테이블의 위치·크기·원본 크기·플래그 |
| xml | XML 리소스의 위치·크기·원본 크기·플래그 |
| bootMetadata | 부팅 메타데이터 리소스 위치·크기·원본 크기·플래그 |
| integrity | 무결성 테이블 리소스 위치·크기·원본 크기·플래그 |
| hasIntegrityTable | 무결성 테이블이 있는지 여부. 검사를 수행한 결과는 아님 |

리소스 객체의 공통 필드는 `offset`, `size`, `originalSize`, `flags`입니다.
표시되는 리소스 중 실제 읽는 것은 헤더와 XML뿐입니다.

## 원문 보존과 파생값

- XML 값은 XML 규칙으로 디코딩한 텍스트입니다. 엔티티/CDATA와 XML 표준 줄바꿈
  정규화는 적용되지만, 공백·대소문자·선행 0을 임의로 변경하지 않습니다.
- 필드가 없으면 null, 존재하지만 비어 있으면 빈 문자열입니다.
- NAME과 DISPLAYNAME은 독립적으로 반환하며, 서로 대체하거나 OS 이름을 추정하지 않습니다.
- XML 숫자도 문자열로 보존합니다. 64비트 크기와 오프셋 역시 문자열로 반환하므로
  JavaScript Number의 정밀도 제한에 영향을 받지 않습니다.
- 이미지 인덱스·이미지 수·헤더의 32비트 필드는 숫자입니다.
- version/buildVersion은 필요한 원문 필드가 모두 비어 있지 않은 ASCII 숫자일 때만
  조합합니다. 원본 개별 필드는 항상 보존합니다.
- FILETIME의 HIGHPART/LOWPART는 원문을 보존하며, 해석 가능한 경우 UTC 문자열도
  제공합니다. 해석할 수 없으면 utc만 null입니다. 파일시스템의 파일 수정 시각과 다릅니다.
- Bootable은 WIM 헤더의 지정 여부입니다. 실제 부팅 성공을 검증하지 않습니다.
- XML BUILD.SPBUILD는 레지스트리 CurrentBuildNumber.UBR와 항상 같다는 의미가 아닙니다.

## 범위

이 파서는 DISM `/Get-ImageInfo` 수준의 정보를 대상으로 합니다.
25H2 같은 DisplayVersion, 업데이트·패키지·드라이버·기능·앱 인벤토리는
이미지 내부 파일과 레지스트리를 추가로 분석해야 하므로 포함하지 않습니다.

표준 MSWIM 시그니처, 208바이트 헤더, 버전 0x10D00/0xE00의 standalone WIM과
같은 구조의 ESD 메타데이터를 지원합니다. 분할 SWM, pipable WIM, 다른 헤더 버전,
압축/solid/spanned XML은 명시적으로 거부합니다.

XML 최대 32MiB, 노드 최대 1,000,000개, 이미지 최대 100,000개로 제한합니다.
잘못된 읽기 범위·UTF-16LE·XML·중복 필드·인덱스·개수 불일치 등을 검사합니다.
XML 읽기에는 JS에서 Wasm 메모리로의 구간 복사가 있습니다.
메모리 사용은 XML 크기에 비례하며 디코딩 문자열과 XML 트리도 생성합니다.
이미지 파일 리소스 전체와 무결성 테이블은 검사하지 않습니다.

## 소스 빌드

Wasm을 다시 빌드할 때는 Rust stable 1.88 이상과 Cargo.lock에 기록된 버전의
wasm-bindgen-cli가 필요합니다. Rust 설치 후 다음 준비를 한 번 실행하세요.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
```

이후 저장소 루트에서 빌드하고 결과물을 검증합니다.

```sh
pnpm --filter @zeroup/wim-info-wasm build
pnpm --filter @zeroup/wim-info-wasm test:wasm
```

`build` 스크립트는 `PATH`에 `$CARGO_HOME/bin`(기본값: `~/.cargo/bin`)을
추가하므로, WebStorm의 실행 환경에 Rust 경로가 없어도 설치된 도구를 찾습니다.

`packages/zeroup__wim-info-wasm/pkg/`에 생성된 `.js`, `.wasm`, `.d.ts` 파일을
모두 소스 변경과 함께 커밋합니다. `target/`은 중간 생성물이므로 Git에서 제외합니다.
설치 시 빌드는 실행하지 않으며, 커밋된 결과물을 사용하는 개발자와 CI에는
Rust 도구가 필요하지 않습니다.

파서 검증은 이 패키지 디렉터리에서 실행합니다.

```sh
cargo test --locked
pnpm test:wasm
```

## 검증

- Rust 테스트 11개 통과: 기본 값, 누락 필드, 상세 필드 전체, 언어, FILETIME,
  정밀도 제한을 넘는 크기, 4GiB 이후 XML 위치, 잘못된 입력을 검증했습니다.
- native/wasm32 대상 Clippy 경고 없이 통과.
- 릴리스 Wasm 빌드 및 JS 바인딩 생성 완료.
- 실제 Wasm과 Node File API를 이용한 `node tests/wasm.mjs` 통과:
  상세 값의 직렬화와 헤더/XML 두 구간만 읽는 것을 확인했습니다.
- Worker 및 화면 스크립트 JavaScript 구문 검사 통과.
- 브라우저 통합 테스트는 tests/browser.cjs에 포함했습니다.
- 실제 설치 WIM 파일은 제공되지 않아 합성 메타데이터로 검증했습니다.

## 참고

- [DISM DismImageInfo](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/dism/dismimageinfo-structure)
- [DISM DismWimCustomizedInfo](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/dism/dismwimcustomizedinfo-structure)
- [Microsoft WIM reader](https://github.com/microsoft/go-winio/blob/main/wim/wim.go)
