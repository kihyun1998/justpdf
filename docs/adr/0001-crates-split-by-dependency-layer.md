# Workspace를 외부 의존성 레이어로 분할

PDF 엔진을 8개 크레이트로 나눈 일차 원칙은 **외부 의존성 계층화**다. 사용자는 필요한 레이어까지만 가져와서 이진 크기와 컴파일 시간을 제어할 수 있다.

## 레이어

- `justpdf-core` — 외부 deps만 (`flate2`, `ttf-parser`, `aes`, 서명 스택 등). PDF 명세 자체(파서/객체모델/암호/서명/텍스트추출).
- `justpdf-render` — + `tiny-skia`. 래스터화 전용.
- `justpdf-formats` — + `zip`, `roxmltree`. non-PDF 입력 포맷(EPUB/XPS/Office/CBZ/MOBI/FB2/SVG/Plaintext) → PDF 어댑터.
- `justpdf-special` — + `qrcode`, `unicode-bidi`. PDF 내부 부가 기능(OCR/Barcode/ZUGFeRD/BiDi/Deskew).
- `justpdf` — `core + render` 통합 진입점. `formats`/`special`은 **의식적으로 제외** — 일차 원칙과 일치하게 "최소만 가져옴".

## 트레이드오프

대안으로 **단일 크레이트 + feature flag 조합**을 검토했다. Rust의 컴파일 단위는 크레이트이므로 크레이트 분할이 컴파일 시간 격리에 더 효과적이고, 사용자가 의존 그래프를 보며 필요한 것만 명시적으로 선택할 수 있어 분할 방식을 선택했다.

## 부수 효과

- `compress-wasm`이 `render`를 의존하지 않는 것은 일차 원칙과 일치 — 브라우저 용량 절약.
- 새 PDF 기능을 추가할 때 "어느 레이어의 deps가 필요한가?"가 위치 결정 기준이 된다.
