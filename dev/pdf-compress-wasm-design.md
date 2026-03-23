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

### Phase B: 스트림 최적화 — ✅ 완료

모든 스트림(폰트, 컨텐츠, 이미지 등)을 최고 압축 레벨로 재압축.

```
구현:
  ✅ B-1. Flate 재압축 — FlateDecode 스트림을 디코딩 → Compression::best()로 재인코딩
  ✅ B-2. 비압축 스트림 감지 강화 — /Length만 있고 /Filter 없는 스트림 모두 처리

테스트:
  ✅ B-T1. FlateDecode 스트림 재압축 → 출력 크기 ≤ 원본 (무손실)
  ✅ B-T2. 재압축 왕복 → 디코딩 결과 원본과 동일
  ✅ B-T3. 이미 best 레벨인 스트림 → 크기 변화 없거나 미미
  ✅ B-T4. 텍스트 PDF 20페이지 → 재압축 후 개선 확인
```

---

### Phase C: 중복 스트림 dedup — ✅ 완료

SHA-256 해시로 동일 스트림 데이터를 감지하고 참조를 통합.

```
구현:
  ✅ C-1. 스트림 데이터 SHA-256 해시 계산
  ✅ C-2. 동일 해시 객체 → 첫 번째만 유지, 나머지 참조 리맵
  ✅ C-3. 리맵 후 GC로 고아 객체 제거

테스트:
  ✅ C-T1. 동일 이미지 2개 임베딩 → dedup 후 1개로 통합, 참조 정상
  ✅ C-T2. 동일 폰트 2개 임베딩 → dedup 후 1개로 통합
  ✅ C-T3. 서로 다른 스트림 → dedup 안 됨 (오탐 없음)
  ✅ C-T4. dedup 후 PDF re-parse → 페이지 수, 텍스트 추출 정상
```

---

### Phase D: 폰트 서브세팅 — ✅ 완료

사용 글리프만 남겨 폰트 크기 50~90% 감소. `font/subset.rs` 코드 활용.

```
구현:
  ✅ D-1. 페이지별 사용 글리프 수집 (컨텐츠 스트림 파싱 Tf/Tj/TJ)
  ✅ D-2. subset_font()로 FontFile2 스트림 서브셋 생성
  ✅ D-3. FontDescriptor의 FontFile2 스트림 교체
  ✅ D-4. Widths 배열 업데이트 (gid_map 기반 리매핑)
  ✅ D-5. CID 폰트 지원 (Type0 → CIDFontType2 → FontFile2)
  ✅ D-6. CIDToGIDMap 업데이트 (서브셋 후 GID 리매핑)
  ✅ D-7. 2-byte CID 문자 코드 추출

주의:
  ✅ CFF 폰트 미지원 (TrueType/glyf만) → 자동 스킵
  ✅ CID 폰트 (CIDFontType2) 처리 완료
  ✅ 서브세팅 실패 시 원본 유지 (안전장치)

테스트:
  ✅ D-T1. Standard 폰트 → 서브세팅 안전하게 스킵
  ✅ D-T2. 서브세팅 비활성 → 처리 안 됨
  ✅ D-T3. 프리셋별 설정 확인 (low=off, medium+=on)
  ✅ D-T4. 이미지 PDF + 서브세팅 파이프라인 크래시 없음
  ✅ D-T5. 비-TrueType 폰트 → 스킵
```

---

### Phase E: 미사용 리소스 제거 — ✅ 완료

페이지에서 실제 참조되지 않는 폰트/이미지/ExtGState를 Resources에서 제거.

```
구현:
  ✅ E-1. 페이지 컨텐츠 스트림 파싱 → 사용된 리소스 이름 수집
         (Tf의 폰트 이름, Do의 XObject 이름, gs의 ExtGState 이름)
  ✅ E-2. Resources dict에서 미사용 항목 제거
  ✅ E-3. GC로 고아 객체 자동 수거

테스트:
  ✅ E-T1. 미사용 리소스 제거 후 PDF 유효
  ✅ E-T2. 제거 후 텍스트 추출 정상
  ✅ E-T3. low 프리셋 → 제거 비활성
  ✅ E-T4. 이미지 PDF → 사용 중인 이미지 보존
```

---

### Phase F: 불필요 데이터 제거 — ✅ 완료

메타데이터, 구조 트리, 썸네일, 임베디드 파일 등 제거.

```
구현:
  ✅ F-1. XMP 메타데이터 스트림 제거 (Catalog /Metadata)
  ✅ F-2. 구조 트리 제거 (Catalog /StructTreeRoot)
  ✅ F-3. 페이지 썸네일 제거 (Page /Thumb)
  ✅ F-4. Output Intent 제거 (Catalog /OutputIntents)
  ✅ F-5. 임베디드 파일 제거 (Catalog /Names → /EmbeddedFiles)
  ✅ F-6. JavaScript 제거 (Catalog /Names → /JavaScript, 페이지 /AA)
  ✅ F-7. /PieceInfo, /LastModified, /MarkInfo 등 앱 전용 데이터 제거
  ✅ F-8. 프리셋별 제거 범위 적용 (high: strip_metadata / extreme: +strip_extras)

테스트:
  ✅ F-T1. low 프리셋 → 메타데이터 제거 안 됨
  ✅ F-T2. high 프리셋 → 메타데이터 제거, PDF 유효
  ✅ F-T3. extreme 프리셋 → extras 포함 제거, PDF 유효
  ✅ F-T4. 제거 후 텍스트 추출 정상
  ✅ F-T5. 프리셋별 strip 설정 확인
  ✅ F-T6. 이미지 PDF → 이미지 보존
```

---

### Phase G: 색상 변환 (Grayscale) — ✅ 완료

RGB/CMYK 이미지를 Grayscale로 변환하여 대폭 크기 감소.

```
구현:
  ✅ G-1. RGB → Grayscale 변환 (luminance: 0.299R + 0.587G + 0.114B)
  ✅ G-2. CMYK → Grayscale 변환
  ✅ G-3. CompressOptions.grayscale: bool 옵션 (기본 false, 명시적 opt-in)
  ✅ G-4. 컨텐츠 스트림 색상 연산자 업데이트 (rg→g, RG→G, k→g, K→G)

테스트:
  ✅ G-T1. RGB 이미지 → Grayscale 변환 후 크기 감소
  ✅ G-T2. Grayscale 변환 후 re-parse 정상
  ✅ G-T3. grayscale=false → 색상 변환 안 됨
  ✅ G-T4. 텍스트 전용 PDF + grayscale=true → 크래시 없음
```

---

### Phase H: Object Stream 압축 — ✅ 완료

작은 딕셔너리 객체를 Object Stream으로 묶어 PDF 1.5+ 최적화.

```
구현:
  ✅ H-1. catalog_ref 무효화 문제 해결 (catalog obj_num 추적)
  ✅ H-2. pack_object_streams() → PackResult (compressed info 포함)
  ✅ H-3. xref stream 생성 — write_xref_stream() (type 0/1/2 entries)
  ✅ H-4. serialize_pdf_with_xref_stream() + build_with_xref_stream()
  ✅ H-5. compress_pdf 파이프라인 연동 완료

테스트:
  ✅ H-T1. eligible 객체 패킹 → 객체 수 감소
  ✅ H-T2. ObjStm 메타데이터 (Type, N, First) 정상
  ✅ H-T3. catalog, pages root, Stream → object stream에 포함 안 됨
```

---

### Phase I: WASM 고급 기능 — ✅ 완료

```
구현:
  ✅ I-1. CompressOptions 전체 WASM 노출 — compress_advanced() 함수
         (jpeg_quality, max_dpi, font_subsetting, remove_unused_resources,
          strip_metadata, strip_extras, grayscale)
  ✅ I-2. CompressStats 전체 필드 노출 — 14개 getter
         (original_size, compressed_size, images_found, images_recompressed,
          images_downscaled, images_skipped, duplicates_removed, objects_removed_gc,
          streams_recompressed, fonts_subsetted, unused_resources_removed,
          metadata_items_stripped, images_grayscaled, ratio)
  ✅ I-3. wasm-pack build --target web 검증 (697KB, getrandom js feature 추가)
  ✅ I-4. npm 패키지 배포 — @kihyun1998/justpdf-compress-wasm@0.1.2
```

---

### Phase J: DPI 정밀 계산 — ✅ 완료

```
구현:
  ✅ J-1. 컨텐츠 스트림에서 cm/q/Q 연산자 추적 → Do 직전 CTM 추출
  ✅ J-2. effective DPI = image_px / (ctm_scale / 72)
  ✅ J-3. CTM 기반 접근법 우선 사용, CTM 없으면 픽셀 기반 폴백
  ✅ J-4. 여러 페이지에서 같은 이미지 사용 시 최대 display size 기준

테스트:
  ✅ J-T1. 200x200pt에 4000x4000px 이미지 → DPI 1440 감지, 150으로 다운스케일
  ✅ J-T2. 전체 페이지 이미지 (300 DPI) → 150 DPI로 다운스케일
  ✅ J-T3. 이미 DPI 예산 내 → 다운스케일 안 함
  ✅ J-T4. CTM 없으면 픽셀 기반 폴백 → 기존 동작과 동일
  ✅ J-T5. 행렬 곱셈 정확성 검증
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
