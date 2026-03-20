# PDF 압축 WASM 설계 문서

> 목표: 브라우저 확장 프로그램에서 사용할 PDF 압축 전용 WASM 모듈 구현

---

## 1. 제품 개요

### 사용 시나리오
1. 사용자가 브라우저에서 PDF 파일을 선택 (또는 드래그 앤 드롭)
2. WASM 모듈이 클라이언트 사이드에서 PDF를 압축
3. 압축된 PDF를 다운로드

### 핵심 요구사항
- **서버 불필요** — 모든 처리가 브라우저 내에서 완료
- **개인정보 보호** — PDF가 외부로 전송되지 않음
- **합리적 속도** — 10MB PDF 기준 수 초 이내
- **유의미한 압축률** — 이미지 많은 PDF에서 50~80% 크기 감소 목표

---

## 2. 크레이트 구조

`justpdf-wasm`(범용)과 분리된 **압축 전용 WASM 크레이트**를 만든다.

```
justpdf/
├── justpdf-core/                    # 기존 — 파싱, 수정, 직렬화
│   └── src/writer/
│       ├── compress.rs              # 신규 — 압축 엔진
│       ├── clean.rs                 # 기존 — dedup
│       ├── modify.rs                # 기존 — DocumentModifier
│       └── encode.rs                # 기존 — FlateDecode
│
├── justpdf-wasm/                    # 기존 — 범용 WASM (변경 없음)
│
└── justpdf-compress-wasm/           # 신규 — 압축 전용 WASM
    ├── Cargo.toml
    └── src/lib.rs                   # compress(), analyze() 만 노출
```

### 왜 분리하는가

| | justpdf-wasm | justpdf-compress-wasm |
|---|---|---|
| 목적 | PDF 뷰어/텍스트 추출/렌더링 | PDF 압축 전용 |
| 의존성 | justpdf-core + **justpdf-render** | justpdf-core만 |
| WASM 번들 | 크다 (tiny-skia 렌더링 엔진 포함) | 작다 (렌더러 없음) |
| API | 10+ 메서드 | 2~3 메서드 |
| 용도 | 범용 PDF 라이브러리 | 브라우저 확장 하나 |

렌더링 엔진(`tiny-skia`)이 WASM 번들의 상당 부분을 차지하는데, 압축에는 불필요.

---

## 3. 현재 상태 분석

### 있는 것 (빌딩 블록)

| 모듈 | 위치 | 설명 |
|------|------|------|
| PDF 파싱 | `justpdf-core/src/parser.rs` | 전체 PDF 객체 로딩 |
| 이미지 XObject 순회 | `examples/extract_images.rs` | Resources → /XObject → /Image 필터링 |
| 이미지 디코딩 | `justpdf-core/src/image/mod.rs` | JPEG/PNG/JP2/JBIG2/CCITT 디코딩 |
| JPEG 인코딩 | `justpdf-render/src/device.rs` | `encode_jpeg(quality)` — image 크레이트 |
| 이미지 리사이즈 | `justpdf-formats/src/cbz/mod.rs` | Lanczos3 다운스케일 |
| FlateDecode 인코딩 | `justpdf-core/src/writer/encode.rs` | `encode_flate()` |
| 중복 객체 제거 | `justpdf-core/src/writer/clean.rs` | `dedup_objects()` + 해시 비교 |
| GC (미사용 객체 제거) | `justpdf-core/src/writer/modify.rs` | `garbage_collect()` |
| Object Stream 압축 | `justpdf-core/src/writer/object_stream.rs` | `pack_object_streams()` |
| 문서 수정 + 저장 | `justpdf-core/src/writer/modify.rs` | `DocumentModifier` |
| WASM 바인딩 | `justpdf-wasm/src/lib.rs` | 기본 읽기/렌더링만 노출 (109줄) |

### 없는 것 (구현 필요)

| 기능 | 설명 |
|------|------|
| **이미지 재인코딩 파이프라인** | 이미지 XObject 찾기 → 디코딩 → JPEG 품질 낮춰 재인코딩 → 스트림 교체 |
| **이미지 다운스케일** | 이미지 DPI 계산 → 목표 DPI로 리사이즈 → 스트림 교체 |
| **통합 compress API** | 위 모든 것을 하나의 함수로 묶는 고수준 API |
| **압축 전용 WASM 크레이트** | `justpdf-compress-wasm` — compress/analyze만 노출 |

---

## 4. 압축 전략

PDF 파일 크기의 대부분은 이미지가 차지한다. 압축 전략을 3단계로 나눈다.

### Level 1: 구조 최적화 (빠름, 안전)
- 미사용 객체 GC (`garbage_collect`)
- 중복 객체 제거 (`dedup_objects`)
- Object Stream 압축 (`pack_object_streams`)
- 비압축 스트림 FlateDecode 적용
- → 기존 코드 조합으로 충분

### Level 2: 이미지 품질 최적화 (주요 크기 감소)
- 이미지 XObject 순회
- 비-JPEG 이미지(PNG, TIFF 등) → JPEG 변환
- JPEG 이미지 → 낮은 품질로 재인코딩
- 이미 충분히 작은 이미지는 스킵 (임계값 기반)
- → 새로 구현 필요

### Level 3: 이미지 해상도 다운스케일 (최대 크기 감소)
- 페이지 내 이미지 표시 크기(points) 대비 실제 픽셀 해상도로 effective DPI 계산
- 목표 DPI 초과 시 다운스케일
- Lanczos3 리샘플링
- → 새로 구현 필요

### 프리셋 (5개)

| 프리셋 | Level | JPEG Quality | Max DPI | 용도 |
|--------|-------|-------------|---------|------|
| `low` | 1 | — | — | 구조만 정리, 화질 손실 없음 |
| `medium` | 1+2 | 75 | — | 적당한 압축, 육안 차이 미미 |
| `high` | 1+2+3 | 65 | 150 | 강한 압축, 웹/이메일용 |
| `extreme` | 1+2+3 | 40 | 96 | 최대 압축, 화질 희생 |
| `custom` | 사용자 지정 | 사용자 지정 | 사용자 지정 | 직접 숫자 입력 |

---

## 5. 아키텍처

```
┌───────────────────────────────────────────────────────┐
│  Browser Extension (JS/TS)                            │
│  ┌─────────────────────────────────────────────────┐  │
│  │  UI: 파일 선택, 프리셋 5개, 진행률, 다운로드     │  │
│  └──────────────────┬──────────────────────────────┘  │
│                     │ Uint8Array + preset              │
│                     │ (Web Worker)                     │
│  ┌──────────────────▼──────────────────────────────┐  │
│  │  justpdf-compress-wasm  (압축 전용 WASM)        │  │
│  │                                                 │  │
│  │  compress(bytes, preset) → CompressResult        │  │
│  │  analyze(bytes)          → AnalyzeResult          │  │
│  └──────────────────┬──────────────────────────────┘  │
│                     │                                 │
│  ┌──────────────────▼──────────────────────────────┐  │
│  │  justpdf-core  (Rust → WASM)                    │  │
│  │                                                 │  │
│  │  writer/compress.rs  ← 핵심 엔진               │  │
│  │    ├── 이미지 XObject 순회                      │  │
│  │    ├── 이미지 디코딩 (image/mod.rs)             │  │
│  │    ├── JPEG 재인코딩 (image 크레이트)            │  │
│  │    ├── 이미지 다운스케일 (image 크레이트)         │  │
│  │    ├── GC + dedup (clean.rs)                    │  │
│  │    └── 직렬화 (serialize.rs)                    │  │
│  └─────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────┘
```

---

## 6. API 설계

### 6.1 Rust 핵심 API (`justpdf-core/src/writer/compress.rs`)

```rust
/// 압축 옵션
pub struct CompressOptions {
    /// JPEG 품질 (1-100). None이면 이미지 재인코딩 안 함.
    pub jpeg_quality: Option<u8>,
    /// 최대 이미지 DPI. None이면 다운스케일 안 함.
    pub max_image_dpi: Option<f64>,
    /// 이 크기(bytes) 이하 이미지는 재인코딩 스킵.
    pub skip_below_bytes: usize,
    /// 구조 최적화 (GC + dedup + object streams)
    pub structural: bool,
    /// 비압축 스트림에 FlateDecode 적용
    pub compress_streams: bool,
}

impl CompressOptions {
    pub fn preset_low() -> Self { ... }
    pub fn preset_medium() -> Self { ... }
    pub fn preset_high() -> Self { ... }
    pub fn preset_extreme() -> Self { ... }
}

/// 압축 결과 통계
pub struct CompressStats {
    pub original_size: usize,
    pub compressed_size: usize,
    pub images_found: usize,
    pub images_recompressed: usize,
    pub images_downscaled: usize,
    pub duplicates_removed: usize,
    pub objects_removed: usize,
}

/// PDF 분석 결과
pub struct AnalyzeResult {
    pub pages: usize,
    pub images: usize,
    pub total_image_bytes: usize,
    pub has_encryption: bool,
}

/// PDF 바이트를 받아서 압축된 PDF 바이트를 반환
pub fn compress_pdf(data: &[u8], options: &CompressOptions) -> Result<(Vec<u8>, CompressStats)>;

/// PDF 분석 (압축 전 미리보기)
pub fn analyze_pdf(data: &[u8]) -> Result<AnalyzeResult>;
```

### 6.2 WASM API (`justpdf-compress-wasm/src/lib.rs`)

```rust
#[wasm_bindgen]
pub fn compress(data: &[u8], preset: &str) -> Result<CompressResult, JsValue>;

#[wasm_bindgen]
pub fn compress_custom(
    data: &[u8],
    jpeg_quality: u8,
    max_dpi: f64,
) -> Result<CompressResult, JsValue>;

#[wasm_bindgen]
pub fn analyze(data: &[u8]) -> Result<AnalyzeResult, JsValue>;

#[wasm_bindgen]
pub struct CompressResult {
    data: Vec<u8>,
    pub original_size: usize,
    pub compressed_size: usize,
    pub images_recompressed: u32,
    pub images_downscaled: u32,
    pub ratio: f64,
}

#[wasm_bindgen]
impl CompressResult {
    /// 압축된 PDF 바이트 (JS에서 Uint8Array로 받음)
    pub fn data(&self) -> Vec<u8>;
}

#[wasm_bindgen]
pub struct AnalyzeResult {
    pub pages: u32,
    pub images: u32,
    pub total_image_bytes: u32,
    pub is_encrypted: bool,
}
```

### 6.3 JS 사용 예시

```javascript
// worker.js — Web Worker에서 실행 (UI 블록 방지)
import init, { compress, analyze } from './pkg/justpdf_compress_wasm.js';

await init();

self.onmessage = async (e) => {
    const { bytes, preset } = e.data;

    // 분석 (선택사항)
    const info = analyze(bytes);
    self.postMessage({ type: 'info', ...info });

    // 압축
    const result = compress(bytes, preset);  // "low"|"medium"|"high"|"extreme"
    self.postMessage({
        type: 'done',
        data: result.data(),
        original_size: result.original_size,
        compressed_size: result.compressed_size,
        ratio: result.ratio,
    });
};
```

```javascript
// main.js — UI 측
const worker = new Worker('worker.js');

fileInput.onchange = async (e) => {
    const file = e.target.files[0];
    const bytes = new Uint8Array(await file.arrayBuffer());

    worker.postMessage({ bytes, preset: 'high' });
};

worker.onmessage = (e) => {
    if (e.data.type === 'info') {
        // "12페이지, 이미지 45개"
        showInfo(e.data);
    }
    if (e.data.type === 'done') {
        // "10.2MB → 2.1MB (79% 감소)"
        showResult(e.data);
        enableDownload(e.data.data);
    }
};
```

```javascript
// custom 옵션 사용 시
import { compress_custom } from './pkg/justpdf_compress_wasm.js';

const result = compress_custom(bytes, 55, 120.0);  // quality=55, maxDpi=120
```

---

## 7. 핵심 엔진 상세 — compress_pdf() 내부 흐름

```
compress_pdf(data, options)
│
├── 1. PdfDocument::from_bytes(data)
│      PDF 파싱, 전체 객체 로딩
│
├── 2. DocumentModifier::from_document(&doc)
│      수정 가능 상태로 변환 (모든 객체 복사)
│
├── 3. 이미지 처리 (options.jpeg_quality가 Some일 때)
│   │
│   ├── 모든 객체 순회
│   │   for (obj_num, obj) in modifier.objects()
│   │     if Subtype == /Image
│   │
│   ├── 스킵 조건 체크
│   │   - 크기 < skip_below_bytes → 스킵
│   │   - SMask(투명도) 있음 → 스킵
│   │   - CMYK 색공간 → 스킵
│   │   - ImageMask → 스킵
│   │
│   ├── 이미지 디코딩
│   │   image::decode_image(raw_data, &dict)
│   │   → DecodedImage { width, height, data: Vec<u8> }
│   │
│   ├── 다운스케일 (options.max_image_dpi가 Some일 때)
│   │   - effective DPI 계산 (CTM 기반 또는 픽셀 크기 기반)
│   │   - 초과 시 image::imageops::resize(Lanczos3)
│   │
│   ├── JPEG 재인코딩
│   │   JpegEncoder::new_with_quality(quality).encode(pixels)
│   │
│   └── 스트림 교체
│       - dict: /Filter → /DCTDecode, /Width, /Height 업데이트
│       - data: 새 JPEG 바이트로 교체
│       - 원본보다 커지면 교체 취소 (안전장치)
│
├── 4. 구조 최적화 (options.structural이 true일 때)
│   ├── garbage_collect() — 미사용 객체 제거
│   ├── clean_objects() — 중복 제거 + null 정리 + 재번호
│   └── pack_object_streams() — 작은 객체 묶기
│
├── 5. 비압축 스트림 FlateDecode (options.compress_streams가 true일 때)
│   └── Filter 없는 스트림 → encode_flate() 적용
│
└── 6. serialize_pdf() → Vec<u8>
       최종 PDF 바이트 출력
```

---

## 8. 이미지 DPI 계산

이미지의 effective DPI를 알려면 해당 이미지가 페이지에서 어떤 크기로 그려지는지 알아야 한다.

```
effective_dpi = image_pixels / display_size_in_inches
             = image_pixels / (display_size_in_points / 72)
```

### 접근법 A (정확, 권장)
- 이미지 XObject를 참조하는 페이지의 컨텐츠 스트림을 파싱
- `Do /ImX` 직전의 CTM에서 스케일 추출
- `image_width_px / (ctm_scale_x / 72)` = effective DPI
- 이미 컨텐츠 스트림 인터프리터가 완성되어 있으므로 활용 가능

### 접근법 B (간단, 보수적)
- CTM 추적 없이, 이미지 픽셀 크기만으로 판단
- 사용자가 "최대 픽셀 수"를 지정 (예: max_pixels = 2048)
- DPI 계산이 정확하지 않지만 구현이 단순

### 구현 순서
- Phase A에서는 접근법 B (간단) 로 시작
- Phase C에서 접근법 A (CTM 기반) 로 업그레이드

---

## 9. WASM 제약사항 & 대응

| 제약 | 대응 |
|------|------|
| 싱글 스레드 | rayon/parallel 비활성, 순차 처리. Web Worker로 UI 블록 방지 |
| 메모리 제한 (~2GB) | 전체 로딩 방식. 100MB+ PDF는 경고 |
| 파일 시스템 없음 | 모든 I/O가 `&[u8]` ↔ `Vec<u8>` |
| 느린 초기화 | WASM 모듈 미리 로드 (`init()`) |

### WASM 빌드 호환성

모든 의존성이 pure Rust → **WASM 블로커 없음**:
- `flate2` → `miniz_oxide` (pure Rust 백엔드)
- `jpeg-decoder` → pure Rust
- `image` 크레이트 → pure Rust JPEG 인코더
- `sha2`, `aes` 등 → RustCrypto (pure Rust)

### 번들 크기 (예상)
- justpdf-wasm (현재, 렌더러 포함): ~2-3MB
- justpdf-compress-wasm (렌더러 없음): ~1-1.5MB
- `image` 크레이트 JPEG 인코더 추가: ~500KB 증가

---

## 10. 의존성 변경

### justpdf-core/Cargo.toml — 추가
```toml
image = { version = "0.25", default-features = false, features = ["jpeg", "png"] }
```
현재 `jpeg-decoder`(디코딩)만 있고 JPEG **인코딩**이 없음.
`image` 크레이트로 인코딩 + 리사이즈 모두 해결.

### justpdf-compress-wasm/Cargo.toml — 신규
```toml
[package]
name = "justpdf-compress-wasm"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
justpdf-core = { path = "../justpdf-core" }
wasm-bindgen = "0.2"
js-sys = "0.3"
```

> `justpdf-render` 의존성 없음 — 번들 크기 최소화

---

## 11. 파일 변경 목록

| 파일 | 변경 |
|------|------|
| `justpdf-core/Cargo.toml` | `image` 크레이트 의존성 추가 |
| `justpdf-core/src/writer/mod.rs` | `pub mod compress;` 추가 |
| `justpdf-core/src/writer/compress.rs` | **신규** — 핵심 압축 엔진 |
| `Cargo.toml` (workspace) | `justpdf-compress-wasm` 멤버 추가 |
| `justpdf-compress-wasm/Cargo.toml` | **신규** — 크레이트 매니페스트 |
| `justpdf-compress-wasm/src/lib.rs` | **신규** — WASM 바인딩 |

---

## 12. 구현 우선순위

```
Phase A: 핵심 압축 엔진
  1. justpdf-core/src/writer/compress.rs
     - compress_pdf() — 이미지 순회 + JPEG 재인코딩 + 구조 최적화
     - analyze_pdf() — PDF 분석
     - 프리셋 4개 (low/medium/high/extreme) + custom
  2. 단위 테스트 (fixture PDF로 왕복 검증)

Phase B: 압축 전용 WASM 크레이트
  3. justpdf-compress-wasm/ 생성
  4. compress(), compress_custom(), analyze() 바인딩
  5. wasm-pack build --target web 검증

Phase C: 이미지 다운스케일
  6. DPI 계산 로직 (CTM 기반)
  7. 리사이즈 + 재인코딩 통합

Phase D: 고급 기능
  8. 진행률 콜백 (wasm_bindgen closure)
  9. 비압축 스트림 자동 FlateDecode
```

---

## 13. 리스크 & 결정 사항

| 항목 | 결정 |
|------|------|
| CMYK JPEG | 스킵 — 색상 변환 손실 위험 |
| SMask(투명도) 있는 이미지 | 스킵 — JPEG은 알파 미지원 |
| 인라인 이미지 (BI/ID/EI) | Phase A에서는 XObject만, 인라인은 나중에 |
| 암호화된 PDF | 에러 반환 (비밀번호 옵션은 나중에) |
| 재인코딩 후 원본보다 커지면 | 교체 취소 (안전장치) |
| ImageMask (1bpp 마스크) | 스킵 — 이미지가 아닌 마스크 |
