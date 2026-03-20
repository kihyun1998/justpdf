# PDF 압축 WASM 설계 문서

> 목표: 브라우저 확장 프로그램에서 사용할 PDF 압축 전용 WASM 모듈 구현
> Ghostscript급 압축률을 pure Rust/WASM으로 달성

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
- **유의미한 압축률** — 이미지 PDF 50~80%, 텍스트 PDF 20~50% 크기 감소 목표

---

## 2. 크레이트 구조

```
justpdf/
├── justpdf-core/                    # 기존 — 파싱, 수정, 직렬화
│   └── src/writer/
│       ├── compress.rs              # 압축 엔진 (모든 기법 통합)
│       ├── clean.rs                 # 기존 — dedup
│       ├── modify.rs                # 기존 — DocumentModifier
│       └── encode.rs                # 기존 — FlateDecode
│
├── justpdf-wasm/                    # 기존 — 범용 WASM (변경 없음)
│
└── justpdf-compress-wasm/           # 압축 전용 WASM (렌더러 없음, 번들 작음)
    ├── Cargo.toml
    └── src/lib.rs                   # compress(), analyze() 만 노출
```

---

## 3. 업계 표준 비교

> iLovePDF, SmallPDF 등 온라인 도구는 내부적으로 Ghostscript 계열 엔진 사용.

| 기법 | Ghostscript | qpdf | 우리 (현재) | 우리 (목표) |
|------|:-----------:|:----:|:----------:|:----------:|
| 이미지 JPEG 재인코딩 | ✓ | ✓ | ✓ | ✓ |
| 이미지 다운샘플링 | ✓ | — | ✓ | ✓ |
| 폰트 서브세팅 | ✓ | — | — | ✓ |
| 폰트 스트림 재압축 | ✓ | — | — | ✓ |
| Flate 재압축 (최고 레벨) | ✓ | ✓ | — | ✓ |
| 중복 스트림 dedup | ✓ | — | — | ✓ |
| 미사용 리소스 제거 | ✓ | — | — | ✓ |
| 메타데이터/구조 제거 | ✓ | — | — | ✓ |
| Object Stream 압축 | — | ✓ | — | ✓ |
| 색상 → Grayscale | ✓ | — | — | ✓ |
| GC (미사용 객체) | ✓ | ✓ | ✓ | ✓ |
| 비압축 → FlateDecode | ✓ | ✓ | ✓ | ✓ |

---

## 4. 프리셋별 적용 기법

| 기법 | low | medium | high | extreme |
|------|:---:|:------:|:----:|:-------:|
| **이미지** | | | | |
| JPEG 재인코딩 | — | q75 | q65 | q40 |
| 이미지 다운스케일 | — | — | 150dpi | 96dpi |
| RGB → Grayscale | — | — | — | 옵션 |
| **폰트** | | | | |
| 폰트 서브세팅 | — | ✓ | ✓ | ✓ |
| **스트림** | | | | |
| 비압축 → FlateDecode | ✓ | ✓ | ✓ | ✓ |
| Flate 재압축 (best) | ✓ | ✓ | ✓ | ✓ |
| **구조** | | | | |
| GC (미사용 객체) | ✓ | ✓ | ✓ | ✓ |
| 중복 스트림 dedup | ✓ | ✓ | ✓ | ✓ |
| 미사용 리소스 제거 | — | ✓ | ✓ | ✓ |
| **제거** | | | | |
| 메타데이터 (XMP 등) | — | — | ✓ | ✓ |
| 구조 트리/썸네일 | — | — | ✓ | ✓ |
| 임베디드 파일 | — | — | — | ✓ |
| JavaScript/액션 | — | — | — | ✓ |
| Output Intent/ICC | — | — | ✓ | ✓ |
| **정보 손실** | 없음 | 최소 | 보통 | 큼 |

---

## 5. 구현 로드맵

### Phase A: 핵심 압축 엔진 — ✅ 완료

```
[v0.1 — 커밋 fa3bfd1]
  ✅ A-1. 이미지 JPEG 재인코딩 (quality 조절)
  ✅ A-2. 비-JPEG → JPEG 변환 (PNG, Raw 등)
  ✅ A-3. 이미지 다운스케일 (max DPI, Lanczos3)
  ✅ A-4. 비압축 스트림 FlateDecode 자동 적용
  ✅ A-5. GC — 미사용 객체 제거
  ✅ A-6. 프리셋 4개 (low/medium/high/extreme) + custom
  ✅ A-7. compress_pdf(), analyze_pdf() API
  ✅ A-8. justpdf-compress-wasm WASM 바인딩
  ✅ A-9. compress_pdf CLI 예제
  ✅ A-10. 단위 테스트 13개
```

---

### Phase B: 스트림 최적화 — 미구현

모든 스트림(폰트, 컨텐츠, 이미지 등)을 최고 압축 레벨로 재압축.

```
구현:
  - [ ] B-1. Flate 재압축 — FlateDecode 스트림을 디코딩 → Compression::best()로 재인코딩
  - [ ] B-2. 비압축 스트림 감지 강화 — /Length만 있고 /Filter 없는 스트림 모두 처리

테스트:
  - [ ] B-T1. FlateDecode 스트림 재압축 → 출력 크기 ≤ 원본 (무손실)
  - [ ] B-T2. 재압축 왕복 → 디코딩 결과 원본과 동일
  - [ ] B-T3. 이미 best 레벨인 스트림 → 크기 변화 없거나 미미
  - [ ] B-T4. 실물 테스트: translated_33_45.pdf low 프리셋 → v0.1 대비 개선
```

---

### Phase C: 중복 스트림 dedup — 미구현

SHA-256 해시로 동일 스트림 데이터를 감지하고 참조를 통합.

```
구현:
  - [ ] C-1. 스트림 데이터 SHA-256 해시 계산
  - [ ] C-2. 동일 해시 객체 → 첫 번째만 유지, 나머지 참조 리맵
  - [ ] C-3. 리맵 후 GC로 고아 객체 제거

테스트:
  - [ ] C-T1. 동일 이미지 2개 임베딩 → dedup 후 1개로 통합, 참조 정상
  - [ ] C-T2. 동일 폰트 2개 임베딩 → dedup 후 1개로 통합
  - [ ] C-T3. 서로 다른 스트림 → dedup 안 됨 (오탐 없음)
  - [ ] C-T4. dedup 후 PDF re-parse → 페이지 수, 텍스트 추출 정상
```

---

### Phase D: 폰트 서브세팅 — 미구현

사용 글리프만 남겨 폰트 크기 50~90% 감소. `font/subset.rs` 코드 활용.

```
구현:
  - [ ] D-1. 페이지별 사용 글리프 수집 (컨텐츠 스트림 파싱 + ToUnicode/Encoding)
  - [ ] D-2. subset_font()로 FontFile2 스트림 서브셋 생성
  - [ ] D-3. FontDescriptor의 FontFile2 스트림 교체
  - [ ] D-4. Widths 배열 업데이트

주의:
  - CFF 폰트 미지원 (TrueType/glyf만)
  - CID 폰트는 별도 처리 필요
  - 서브세팅 실패 시 원본 유지 (안전장치)

테스트:
  - [ ] D-T1. TrueType 폰트 임베딩 PDF → 서브세팅 후 크기 감소
  - [ ] D-T2. 서브세팅 후 텍스트 추출 → 원본과 동일
  - [ ] D-T3. 서브세팅 후 렌더링 → 글리프 깨짐 없음
  - [ ] D-T4. CFF 폰트 → 서브세팅 스킵, 원본 유지
  - [ ] D-T5. 실물 테스트: translated_33_45.pdf → 폰트 크기 대폭 감소
```

---

### Phase E: 미사용 리소스 제거 — 미구현

페이지에서 실제 참조되지 않는 폰트/이미지/ExtGState를 Resources에서 제거.

```
구현:
  - [ ] E-1. 페이지 컨텐츠 스트림 파싱 → 사용된 리소스 이름 수집
         (Tf의 폰트 이름, Do의 XObject 이름, gs의 ExtGState 이름)
  - [ ] E-2. Resources dict에서 미사용 항목 제거
  - [ ] E-3. GC로 고아 객체 자동 수거

테스트:
  - [ ] E-T1. 폰트 2개 등록, 1개만 사용 → 미사용 폰트 제거됨
  - [ ] E-T2. 이미지 3개 등록, 2개만 사용 → 미사용 이미지 제거됨
  - [ ] E-T3. 제거 후 PDF re-parse → 사용 중인 리소스 정상 동작
  - [ ] E-T4. Form XObject 내부 리소스 → 재귀적으로 체크
```

---

### Phase F: 불필요 데이터 제거 — 미구현

메타데이터, 구조 트리, 썸네일, 임베디드 파일 등 제거.

```
구현:
  - [ ] F-1. XMP 메타데이터 스트림 제거 (Catalog /Metadata)
  - [ ] F-2. 구조 트리 제거 (Catalog /StructTreeRoot)
  - [ ] F-3. 페이지 썸네일 제거 (Page /Thumb)
  - [ ] F-4. Output Intent 제거 (Catalog /OutputIntents)
  - [ ] F-5. 임베디드 파일 제거 (Catalog /Names → /EmbeddedFiles)
  - [ ] F-6. JavaScript 제거 (Catalog /Names → /JavaScript, 페이지 /AA)
  - [ ] F-7. /PieceInfo, /LastModified 등 앱 전용 데이터 제거
  - [ ] F-8. 프리셋별 제거 범위 적용 (high: F-1~4,7 / extreme: F-1~7 전부)

테스트:
  - [ ] F-T1. XMP 있는 PDF → 제거 후 크기 감소, PDF 정상
  - [ ] F-T2. Tagged PDF (StructTreeRoot) → 제거 후 크기 감소
  - [ ] F-T3. 썸네일 있는 PDF → 제거 후 크기 감소
  - [ ] F-T4. 임베디드 파일 있는 PDF → extreme에서만 제거
  - [ ] F-T5. JavaScript 있는 PDF → extreme에서만 제거
  - [ ] F-T6. 제거 후 re-parse → 페이지/텍스트 정상
```

---

### Phase G: 색상 변환 (Grayscale) — 미구현

RGB/CMYK 이미지를 Grayscale로 변환하여 대폭 크기 감소.

```
구현:
  - [ ] G-1. RGB → Grayscale 변환 (66% 감소)
  - [ ] G-2. CMYK → Grayscale 변환 (75% 감소)
  - [ ] G-3. extreme 프리셋에서 옵션으로만 제공 (CompressOptions.grayscale: bool)
  - [ ] G-4. 컨텐츠 스트림의 색상 연산자도 업데이트 (rg→g, RG→G 등)

테스트:
  - [ ] G-T1. RGB 이미지 → Grayscale 변환 후 크기 ~66% 감소
  - [ ] G-T2. 변환 후 이미지 디코딩 → 그레이스케일 픽셀 정상
  - [ ] G-T3. grayscale=false → 색상 변환 안 됨
  - [ ] G-T4. 이미 Grayscale 이미지 → 스킵
```

---

### Phase H: Object Stream 압축 — 미구현

작은 딕셔너리 객체를 Object Stream으로 묶어 PDF 1.5+ 최적화.

```
구현:
  - [ ] H-1. catalog_ref 무효화 문제 해결
         (방법: build() 직전에 pack, 또는 catalog obj_num 추적)
  - [ ] H-2. pack_object_streams() 연동
  - [ ] H-3. xref stream 생성 (object stream 사용 시 필수)

테스트:
  - [ ] H-T1. 텍스트 PDF → object stream 적용 후 크기 감소
  - [ ] H-T2. 적용 후 re-parse → 모든 객체 접근 정상
  - [ ] H-T3. catalog, pages root → object stream에 포함 안 됨
```

---

### Phase I: WASM 고급 기능 — 미구현

```
구현:
  - [ ] I-1. CompressOptions 전체 WASM 노출 (세밀한 제어)
  - [ ] I-2. CompressStats 전체 필드 노출
  - [ ] I-3. wasm-pack build --target web 검증
  - [ ] I-4. npm 패키지 준비

테스트:
  - [ ] I-T1. wasm-pack build 성공
  - [ ] I-T2. JS에서 compress("high") 호출 → 유효한 PDF 반환
  - [ ] I-T3. JS에서 analyze() → 정확한 페이지/이미지 수
  - [ ] I-T4. JS에서 compress_custom(quality, dpi) → 동작
```

---

### Phase J: DPI 정밀 계산 — 미구현

```
구현:
  - [ ] J-1. 컨텐츠 스트림에서 Do /ImX 직전 CTM 추출
  - [ ] J-2. effective DPI = image_px / (ctm_scale / 72)
  - [ ] J-3. 현재 접근법 B (픽셀 기반) → 접근법 A (CTM 기반) 교체

테스트:
  - [ ] J-T1. 200x200pt에 4000x4000px 이미지 → DPI 1440 감지
  - [ ] J-T2. 전체 페이지 이미지 → DPI 정확 계산
  - [ ] J-T3. 여러 페이지에 다른 크기로 사용 → 최대 DPI 기준
```

---

## 6. 실물 테스트 결과

### v0.1 (Phase A) 기준

**interest_free_loans_brochure.pdf** (0.51 MB, 이미지 위주)

| 프리셋 | 출력 크기 | 감소율 | 재인코딩 | 다운스케일 |
|--------|----------|--------|---------|-----------|
| low | 0.51 MB | 0.3% | 0 | 0 |
| medium | 0.50 MB | 4.1% | 2 | 0 |
| high | 0.45 MB | 12.8% | 4 | 1 |
| extreme | 0.29 MB | **43.7%** | 4 | 1 |

**translated_33_45.pdf** (69.4 MB, 텍스트/폰트 위주)

| 프리셋 | 출력 크기 | 감소율 | 비고 |
|--------|----------|--------|------|
| high | 69.26 MB | 0.2% | 이미지 9.3MB(13%)뿐, 나머지 폰트/텍스트 |
| extreme | 69.05 MB | 0.6% | Phase B~D 적용 시 대폭 개선 예상 |

### Phase 완료 시 기대 효과

| Phase | interest_free (이미지) | translated (텍스트) |
|-------|:---------------------:|:-------------------:|
| A (현재) | 43.7% | 0.6% |
| + B (Flate 재압축) | ~45% | ~5% |
| + C (dedup) | ~45% | ~10% |
| + D (폰트 서브세팅) | ~45% | ~30~50% |
| + E (미사용 리소스) | ~46% | ~35~55% |
| + F (데이터 제거) | ~48% | ~40~60% |

---

## 7. 리스크 & 결정 사항

| 항목 | 결정 |
|------|------|
| CMYK JPEG | 스킵 — 색상 변환 손실 위험 |
| SMask(투명도) 있는 이미지 | 스킵 — JPEG은 알파 미지원 |
| 인라인 이미지 (BI/ID/EI) | XObject만 처리, 인라인은 후순위 |
| 암호화된 PDF | 에러 반환 |
| 재인코딩 후 원본보다 커지면 | 교체 취소 (안전장치) |
| 폰트 서브세팅 실패 | 원본 유지 (안전장치) |
| CFF 폰트 서브세팅 | 미지원 → TrueType만 |
| Grayscale 변환 | extreme + 옵션 플래그 필요 |
| 구조 트리 제거 | 접근성 손실 — high/extreme만 |
| clean_objects renumbering | catalog_ref 무효화 → GC만 사용 (Phase H에서 해결) |

---

## 8. WASM 제약사항

| 제약 | 대응 |
|------|------|
| 싱글 스레드 | Web Worker로 UI 블록 방지 |
| 메모리 ~2GB | 100MB+ PDF는 경고 |
| 파일 시스템 없음 | `&[u8]` ↔ `Vec<u8>` |
| 번들 크기 | 렌더러 제외, ~1-1.5MB 예상 |

모든 의존성 pure Rust → WASM 블로커 없음.
