# Aibro

Rust + cef-rs 기반의 AI용 로컬 Chromium 브라우저. 사람이 볼 수 있는 Chromium 창과 AI가 사용할 수 있는 JSON API / MCP 서버를 함께 제공한다. AI 모델이나 API 키는 내장하지 않는다. 연결한 에이전트가 판단하고 Aibro가 브라우저 명령을 실행한다.

## 시작

현재 구현·검증 대상은 **Windows x64**다. Rust MSVC 툴체인과 Visual Studio C++ Build Tools(CMake 포함)가 필요하다. 첫 빌드는 CEF 런타임을 내려받으므로 시간과 수 GB의 디스크 공간이 필요하다. Chromium 업데이트는 의존성 버전을 명시적으로 올리고 테스트한다.

```powershell
.\scripts\build.ps1
.\scripts\run.ps1
.\scripts\call.ps1 '{"op":"tabs"}'
.\scripts\call.ps1 '{"op":"snapshot","tab":1}'
```

실행 번들은 `target/bundle/`에 있다. `scripts/run.ps1`을 사용하면 프로필·연결 정보·로그가 프로젝트의 `.aibro/`에 저장된다. 실행 파일을 직접 실행하면 실행 파일 옆 `.aibro/`를 쓴다. 별도 저장 위치는 `--aibro-data-dir=<절대 경로>`로 지정한다. CEF의 Windows 재실행 과정에서도 유지되도록 스크립트는 이 인자를 전달한다.

`cargo run`은 브라우저 실행 방법이 아니다. Windows CEF 샌드박스에 필요한 bootstrap executable과 DLL을 `build.ps1`이 패키징한다. `--no-sandbox`는 사용하지 않는다. 빌드를 다시 하기 전에 실행 중인 브라우저를 종료한다.

```powershell
.\scripts\call.ps1 '{"op":"shutdown"}'
```

## AI 작업 방식

1. `tabs`로 탭 ID를 얻는다. 현재는 CEF 브라우저 인스턴스마다 별도 Chromium 창이 열린다.
2. `snapshot`으로 본문, 제목, URL, 폼 상태, 대상 `ref`를 읽는다.
3. 최신 스냅샷의 `ref`로 `click`, `fill`, `select`를 실행한다.
4. 동작 결과로 반환된 새 스냅샷에서 다음 대상을 선택한다.

**새 스냅샷은 이전 ref를 전부 무효화한다.** 문서가 이동하거나 대상이 제거되어도 기존 참조는 실패한다. 좌표는 AI가 추측하지 않고 참조된 요소에서 계산한다. 클릭 직전에 스크롤·가림·비활성 상태를 확인하고 CDP 입력 이벤트로 실행한다. 폼 입력은 포커스와 선택 영역을 설정한 뒤 Chromium의 `Input.insertText`를 사용한다.

```json
{"op":"navigate","tab":1,"url":"https://example.com"}
{"op":"click","tab":1,"target":"<최신 snapshot의 ref>"}
{"op":"fill","tab":1,"target":"<입력 요소 ref>","text":"검색어"}
```

| op | 인자 | 결과 |
|---|---|---|
| tabs | 없음 | 탭 ID, URL, 로딩 여부 |
| open | url | 새 탭 ID와 page |
| navigate | tab, url | 스냅샷 |
| snapshot | tab | 스냅샷 |
| click | tab, target | 스냅샷 |
| fill | tab, target, text | 스냅샷 |
| select | tab, target, value | 스냅샷 |
| press | tab, key | 스냅샷 |
| scroll | tab, y, x(선택) | 스냅샷 |
| screenshot | tab | PNG base64 |
| back / reload | tab | 스냅샷 |
| close | tab | 종료 요청 확인 |
| shutdown | 없음 | 전체 종료 요청 확인 |

`press`는 Enter, Tab, Escape, Backspace, Delete, ArrowLeft, ArrowUp, ArrowRight, ArrowDown을 지원한다. URL은 HTTP(S)와 `about:blank`만 받는다. 알 수 없는 필드는 오류다. 실패한 동작은 자동 재시도하지 않는다. 전송·결제 등의 중복 실행을 막기 위해 현재 페이지를 읽고 다시 판단한다.

## MCP 연결

`target/bundle/aibro-agent.exe`는 표준 입출력 MCP 서버이며 별도로 실행 중인 브라우저에 연결한다. MCP 초기화 프로토콜은 2025-11-25 및 이전 3개 버전을 지원한다. 클라이언트 설정 예:

```json
{
  "mcpServers": {
    "aibro": {
      "command": "C:\\work\\aibro\\target\\bundle\\aibro-agent.exe",
      "args": ["--data-dir", "C:\\work\\aibro\\.aibro"]
    }
  }
}
```

도구 이름은 `browser`, 인자는 위 표의 JSON 명령이다. 스크린샷은 MCP image content로 전달된다. 이 저장소는 클라이언트의 전역 MCP 설정을 수정하지 않는다.

같은 Rust 실행 파일은 CLI로도 쓸 수 있다.

```powershell
.\target\bundle\aibro-agent.exe --data-dir C:\work\aibro\.aibro --call '{"op":"tabs"}'
```

## HTTP API

`.aibro/connection.json`에 임의 포트의 loopback URL과 실행마다 바뀌는 토큰을 기록한다. `POST /command`에 `Authorization: Bearer <token>`을 붙여 JSON을 전송한다. 응답은 `{"ok":true,"id":"...","data":...}` 또는 `{"ok":false,"id":"...","error":"..."}`다. 인증된 `GET /health`도 제공한다.

API는 127.0.0.1에만 바인딩하고 Origin 헤더가 있는 요청은 거부한다. 웹 페이지에 네이티브 실행 권한이나 제어 토큰을 노출하지 않는다. 페이지 참조 테이블은 CDP isolated world에 보관한다. 페이지 내용은 에이전트 지시문이 아니라 외부 데이터로 취급해야 한다. 토큰과 프로필이 있는 `.aibro/`는 공유하거나 커밋하지 않는다.

`actions.jsonl`에는 작업 ID, op, 성공 여부, 소요 시간만 저장한다. 입력값과 페이지 본문은 이 작업 로그에 기록하지 않는다. Chromium 자체 진단 로그는 `chromium.log`에 별도로 남는다.

## CLI와 MCP 성능 비교

둘 다 Rust로 구현되어 있고 동일한 로컬 브라우저 HTTP API를 호출한다. CLI는 명령마다 새 프로세스를 실행하고, MCP는 프로세스를 유지한다. 반복 제어에는 MCP를 기본으로 사용하고, 터미널 점검에는 CLI를 쓴다.

실행 중인 브라우저에서 `node scripts/benchmark.mjs`로 비교할 수 있다. 이 스크립트는 탭 목록과 페이지 스냅샷만 읽고, 페이지 내용은 출력하거나 저장하지 않는다. MCP 초기화와 AI 추론 시간은 측정에서 제외한다. 수치는 페이지와 컴퓨터 상태에 따라 달라진다.

개발 환경에서의 1회 측정(각 모드를 번갈아 실행):

| 명령 | 모드별 표본 수 | Rust CLI 중앙값 | 유지 중인 MCP 중앙값 |
|---|---:|---:|---:|
| tabs | 30 | 9.61 ms | 7.96 ms |
| snapshot | 10 | 52.40 ms | 42.85 ms |

이는 전송 방식과 프로세스 실행 비용의 비교다. 네트워크 탐색, 페이지 로딩, AI가 다음 행동을 결정하는 시간까지 포함한 전체 작업 속도를 뜻하지 않는다.

## 구조와 유지보수

- `src/runtime.rs`: CEF 생명주기, UI 스레드, 탭, DevTools 응답 라우팅.
- `src/agent.rs`: 명령 검증, 페이지 관찰과 동작.
- `src/js/snapshot.js`: 제한된 크기의 DOM 요약과 참조 관리.
- `src/api.rs`: 인증된 직렬 명령 처리와 작업 로그.
- `src/bin/aibro-agent.rs`: 브라우저와 분리된 MCP / CLI 전송 계층.
- `src/bin/bundle.rs`: cef-rs 공식 Windows 번들러 사용.

CEF 객체는 UI 스레드에만 둔다. 네트워크 스레드는 채널로 요청하고, DevTools 응답은 명령 ID로 매칭한다. UI 작업과 CDP 대기에는 만료 시간이 있다. 참조 없는 임의 JavaScript 실행은 외부 프로토콜에 제공하지 않는다.

```powershell
# build.ps1과 동일한 CEF 캐시 사용
$env:CEF_PATH = Join-Path $env:LOCALAPPDATA 'aibro\cef'
cargo test --lib
cargo fmt --check
# 실행 중인 브라우저 필요. Node는 이 개발 검증에만 사용하고 패키지는 필요 없다.
node scripts/smoke.mjs
```

## 현재 범위

이 버전은 Windows에서 실제 동작하는 에이전트 브라우저의 기반이다. DOM 요약은 완전한 접근성 트리가 아니다. 본문은 16,000자, 요소는 600개까지 제공한다. 메인 문서와 열린 shadow root를 읽고 조작한다. iframe은 목록만 제공하며 내부 조작, 닫힌 shadow root, 캔버스 내부 의미 해석은 구현하지 않았다. 날짜·파일 입력, 업로드·다운로드 관리, 브라우저 권한·JS 대화상자 자동 처리, 모바일·다른 OS 패키징도 후속 범위다.

탐색 동작은 현재 로드 완료를 기다린다(최대 10초). 이후에 비동기로 변경되는 페이지는 새 스냅샷으로 상태를 확인한다. 이미 실행된 동작이 로딩 대기 중 실패할 수 있으므로 오류 응답이 동작 취소를 뜻하지는 않는다.

CEF Chromium / cef-rs의 라이선스와 배포 고지는 런타임 번들에 함께 보존한다. cef-rs: https://github.com/tauri-apps/cef-rs

## 라이선스

Aibro 소스는 MIT 또는 Apache-2.0 중 선택하여 사용할 수 있다. `LICENSE-MIT`와 `LICENSE-APACHE`를 참고한다. cef-rs의 별도 고지는 `third_party/cef-rs/`에 보존한다. Chromium 런타임에는 별도의 오픈소스 고지가 적용된다.
