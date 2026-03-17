# justpdf Roadmap

> Pure Rust PDF engine - MuPDF에 필적하는 기능을 순수 Rust로 구현
> 참조: [docs/mupdf-feature-analysis.md](docs/mupdf-feature-analysis.md)

---

## 설계 원칙

- **Pure Rust** - C 바인딩 없이 순수 Rust로 작성. unsafe 최소화.
- **Zero-copy where possible** - 대용량 PDF도 메모리 효율적으로 처리
- **Modular crate 구조** - 기능별 독립 crate으로 분리, 필요한 것만 사용 가능
- **PDF 스펙 준수** - PDF 2.0 (ISO 32000-2:2020) 기준
- **Rust ecosystem 활용** - image, rusttype/ab_glyph, flate2, ring 등 성숙한 crate 적극 활용

---

## Crate 구조 (계획)

```
justpdf/
├── justpdf-core        # PDF 객체 모델, 파서, xref, 스트림
├── justpdf-render       # 래스터화 엔진 (경로, 텍스트, 이미지, 블렌딩)
├── justpdf-text         # 텍스트 추출, 구조화 텍스트, 검색
├── justpdf-font         # 폰트 로딩, 서브세팅, 글리프 매핑
├── justpdf-image        # 이미지 디코딩/인코딩 (JPEG, PNG, TIFF, JBIG2, JP2)
├── justpdf-crypto       # 암호화/복호화 (AES, RC4), 해시 (MD5, SHA)
├── justpdf-annot        # 어노테이션, 폼 필드
├── justpdf-sign         # 디지털 서명 (PKCS#7)
├── justpdf-writer       # PDF 생성/저장/증분저장
├── justpdf-color        # 색공간, ICC 프로필, 색 변환
└── justpdf              # 통합 crate (re-export + 고수준 API)
```

---

## Phase 0: 기반 구축

> 목표: 프로젝트 골격 완성, 가장 단순한 PDF를 열고 읽을 수 있는 상태

### 0.1 프로젝트 셋업
- [x] workspace 구성 (Cargo.toml workspace)
- [ ] CI 셋업 (GitHub Actions: build, test, clippy, fmt)
- [x] 테스트 PDF 수집 (공개 PDF 테스트 스위트)
- [ ] 벤치마크 프레임워크 (criterion)

### 0.2 PDF 토크나이저/렉서
- [x] PDF 바이트 스트림 읽기
- [x] 토큰 타입: Number, String (literal/hex), Name, Keyword, Array, Dict
- [x] 주석(comment) 스킵
- [x] 스트림 위치 추적

### 0.3 PDF 객체 모델
- [x] 기본 타입: Null, Bool, Integer, Real, Name, String, Array, Dict, Stream
- [x] Indirect Reference (obj num + gen num)
- [x] 객체 비교, 복제
- [x] Rust enum 기반 타입 안전한 모델

### 0.4 Cross-Reference 파서
- [x] xref 테이블 파싱 (전통 포맷)
- [x] xref 스트림 파싱 (PDF 1.5+)
- [x] trailer 딕셔너리 파싱
- [x] 다중 xref (증분 업데이트) 체인 따라가기
- [x] `%%EOF` 마커로부터 역방향 xref 탐색

### 0.5 기본 스트림 디코딩
- [x] FlateDecode (zlib/deflate)
- [x] ASCIIHexDecode
- [x] ASCII85Decode
- [x] 필터 체인 (다중 필터 순차 적용)
- [x] Predictor 지원 (PNG, TIFF)

### 0.T 테스트 요구사항

**Positive Tests:**
- [x] 유효한 PDF 파일 열기 → 버전 번호 파싱 성공
- [x] 각 객체 타입 (Null, Bool, Int, Real, Name, String, Array, Dict) 파싱 검증
- [x] hex string `<48656C6C6F>` → "Hello" 변환 확인
- [x] literal string 이스케이프 `(Hello\nWorld)` 파싱
- [x] xref 테이블 파싱 → 객체 오프셋 정확성 검증
- [x] xref 스트림 (PDF 1.5+) 파싱 → 객체 위치 일치 확인
- [x] 증분 업데이트된 PDF → 최신 xref 체인 따라가기
- [x] FlateDecode 스트림 → 원본 데이터 복원
- [x] ASCIIHexDecode, ASCII85Decode 왕복 테스트
- [x] 필터 체인 (FlateDecode + ASCIIHexDecode) 순차 디코딩
- [x] Indirect Reference 해석 → 실제 객체 반환

**Negative Tests:**
- [x] 존재하지 않는 파일 → `Error` 반환 (패닉 아님)
- [x] PDF 헤더 없는 파일 (`.txt` 등) → 명확한 에러 메시지
- [x] 잘린(truncated) PDF → 파싱 에러
- [x] xref 오프셋이 파일 크기 초과 → 에러 처리
- [x] 존재하지 않는 객체 번호 참조 → `None` 또는 에러
- [x] 손상된 FlateDecode 스트림 → 디코딩 에러 (패닉 아님)
- [x] 빈 파일 (0 bytes) → 에러
- [x] 순환 참조 (obj A → obj B → obj A) → 무한루프 방지

### 0.E 완료 확인 (직접 실행)
```bash
# PDF 객체 트리 덤프 — 아무 PDF 파일 사용
cargo run --example dump_objects -- your.pdf

# 기대 결과:
# PDF Version: 1.7
# Objects: 142
# obj 1 0: Dict { /Type: /Catalog, /Pages: 2 0 R, ... }
# obj 2 0: Dict { /Type: /Pages, /Count: 5, ... }
# ...

# 특정 객체 조회
cargo run --example dump_objects -- your.pdf --obj 3

# 스트림 디코딩 확인
cargo run --example decode_stream -- your.pdf --obj 5

# 전체 테스트
cargo test -p justpdf-core
```

---

## Phase 1: PDF 읽기 엔진

> 목표: 대부분의 PDF를 파싱하고 페이지 콘텐츠를 해석할 수 있는 상태

### 1.1 페이지 트리
- [x] Page Tree 순회 (Pages → Page)
- [x] 페이지 수 계산
- [x] 상속 속성 해석 (Resources, MediaBox, Rotate 등)
- [x] Page Box: MediaBox, CropBox, BleedBox, TrimBox, ArtBox

### 1.2 리소스 딕셔너리
- [x] Font 리소스 로딩
- [x] XObject 리소스 (Image, Form)
- [x] ColorSpace 리소스
- [x] ExtGState 리소스
- [ ] Pattern 리소스 *(Phase 2로 이동)*
- [ ] Shading 리소스 *(Phase 2로 이동)*
- [ ] Properties (Optional Content) *(Phase 7로 이동)*

### 1.3 컨텐츠 스트림 인터프리터
- [x] Graphics State 관리 (CTM, color, line style, text state)
- [x] 경로 연산자: m, l, c, v, y, h, re
- [x] 경로 페인팅: S, s, f, F, f*, B, B*, b, b*, n
- [x] 클리핑: W, W*
- [x] 텍스트 연산자: BT/ET, Tf, Td, TD, Tm, T*, Tj, TJ, ', "
- [x] 이미지 연산자: Do (XObject), BI/ID/EI (인라인 이미지)
- [x] 색상 연산자: CS/cs, SC/sc/SCN/scn, G/g, RG/rg, K/k
- [x] Graphics State 연산자: q/Q, cm, w, J, j, M, d, ri, i, gs
- [x] Marked Content: BMC/BDC/EMC, MP/DP
- [x] Shading: sh
- [x] Type3 폰트 연산자: d0, d1

### 1.4 추가 스트림 필터
- [x] LZWDecode
- [x] RunLengthDecode
- [ ] CCITTFaxDecode (Group 3, Group 4) *(Phase 2로 이동)*
- [x] DCTDecode (JPEG)
- [ ] JPXDecode (JPEG2000) *(Phase 2로 이동)*
- [ ] JBIG2Decode *(Phase 2로 이동)*
- [ ] Crypt 필터 *(Phase 6으로 이동)*

### 1.5 폰트 기본
- [x] Type1 폰트 (Standard 14 내장)
- [ ] TrueType 폰트 로딩 (글리프 아웃라인) *(Phase 2로 이동)*
- [x] CIDFont (Type0) 기본 (너비 구조)
- [x] CMap 파싱 (ToUnicode CMap)
- [x] ToUnicode 매핑
- [x] 글리프 너비 / 메트릭스
- [x] 인코딩: WinAnsi, MacRoman, StandardEncoding, PDFDocEncoding

### 1.6 이미지 디코딩
- [x] JPEG (DCTDecode)
- [x] PNG-style (FlateDecode + Predictor)
- [ ] JPEG2000 (JPXDecode) *(Phase 2로 이동)*
- [ ] JBIG2 *(Phase 2로 이동)*
- [ ] CCITT Fax *(Phase 2로 이동)*
- [x] 인라인 이미지
- [ ] 이미지 마스크, 소프트 마스크, SMask *(Phase 2로 이동)*

### 1.7 색공간
- [x] DeviceGray, DeviceRGB, DeviceCMYK
- [x] CalGray, CalRGB
- [x] Lab
- [x] Indexed
- [x] Separation, DeviceN (기본)
- [x] 색 변환 (Gray↔RGB↔CMYK)

### 1.T 테스트 요구사항

**Positive Tests:**
- [x] 페이지 수 정확히 반환 (다양한 PDF로 검증)
- [x] 상속된 MediaBox 정확히 해석 (부모 Pages에만 MediaBox 있는 경우)
- [x] 각 Page Box (Media/Crop/Bleed/Trim/Art) 값 읽기
- [x] 컨텐츠 스트림 연산자 파싱 — 알려진 연산자 시퀀스와 비교
- [x] Graphics State push/pop (q/Q) 균형 검증
- [x] 텍스트 연산자 (BT/ET, Tj, TJ) → 텍스트 문자열 추출
- [x] Standard 14 폰트 이름 인식 → 메트릭스 로딩
- [ ] TrueType 임베디드 폰트 → 글리프 너비 정확성 *(Phase 2)*
- [x] ToUnicode CMap → 유니코드 문자열 변환
- [ ] CJK 폰트 (사전 정의 CMap) → 한글/중국어/일본어 텍스트 *(Phase 3)*
- [x] JPEG 이미지 디코딩 → 올바른 크기/채널 수
- [x] 인라인 이미지 (BI/ID/EI) 파싱
- [x] DeviceRGB → DeviceCMYK 색 변환 왕복 테스트
- [x] Indexed 색공간 → 팔레트에서 실제 색상 조회

**Negative Tests:**
- [x] 페이지 번호 범위 초과 (page 999 on 5-page PDF) → 에러
- [x] 누락된 리소스 (Font 참조했는데 없음) → 에러 또는 폴백
- [x] 알 수 없는 연산자 → 무시하고 계속 파싱 (크래시 아님)
- [x] 손상된 컨텐츠 스트림 (갑자기 끊김) → 에러 처리
- [x] ToUnicode 없는 커스텀 인코딩 폰트 → 빈 문자열 또는 폴백
- [x] 지원하지 않는 이미지 필터 → 명확한 에러
- [x] 깊이 중첩된 Form XObject (재귀) → 무한루프 방지

### 1.E 완료 확인 (직접 실행)
```bash
# PDF 페이지 정보 출력 — 아무 PDF 사용
cargo run --example page_info -- your.pdf

# 기대 결과:
# Pages: 12
# Page 1: MediaBox [0 0 612 792], Rotate 0
# Page 2: MediaBox [0 0 612 792], CropBox [50 50 562 742]
# ...

# 컨텐츠 스트림 연산자 트레이스
cargo run --example trace_ops -- your.pdf --page 1

# 기대 결과:
# q
# 1 0 0 1 72 720 cm
# BT /F1 12 Tf (Hello World) Tj ET
# Q

# 폰트 목록
cargo run --example list_fonts -- your.pdf

# 이미지 추출
cargo run --example extract_images -- your.pdf --out-dir ./images/

cargo test -p justpdf-core
```

---

## Phase 2: 렌더링 엔진

> 목표: PDF 페이지를 픽셀로 렌더링 (PNG/이미지 출력)

### 2.0 Phase 1에서 이월된 항목
- [ ] CCITTFaxDecode (Group 3, Group 4) 스트림 필터
- [ ] JPXDecode (JPEG2000) 스트림 필터
- [ ] JBIG2Decode 스트림 필터
- [ ] TrueType 폰트 로딩 (글리프 아웃라인 파싱)
- [ ] JPEG2000 이미지 디코딩
- [ ] JBIG2 이미지 디코딩
- [ ] CCITT Fax 이미지 디코딩
- [ ] 이미지 마스크, 소프트 마스크, SMask
- [ ] Pattern 리소스 로딩
- [ ] Shading 리소스 로딩

### 2.1 Device 추상화
- [ ] Device trait 정의 (fill_path, stroke_path, fill_text, fill_image, ...)
- [ ] Pixmap (RGBA 버퍼) 구현
- [ ] Display List (명령 기록/재생)
- [ ] BBox Device (바운딩 박스 계산)

### 2.2 경로 래스터화
- [ ] 직선/베지에 곡선 → 엣지 변환
- [ ] Scanline 래스터화 (Even-Odd / Winding)
- [ ] Anti-aliasing (서브픽셀 샘플링)
- [ ] Line Cap (Butt, Round, Square)
- [ ] Line Join (Miter, Round, Bevel)
- [ ] Dash Pattern
- [ ] Stroke → Fill 변환

### 2.3 텍스트 렌더링
- [ ] 글리프 아웃라인 추출 (FreeType 또는 Rust 구현)
- [ ] 글리프 래스터화
- [ ] 글리프 캐싱
- [ ] 텍스트 위치 계산 (Tm, Td, kerning)
- [ ] CJK 텍스트

### 2.4 이미지 렌더링
- [ ] 이미지 → Pixmap 디코딩
- [ ] 어핀 변환 적용 (스케일, 회전, 기울임)
- [ ] 보간 (Nearest, Bilinear, Bicubic)
- [ ] 이미지 마스크 적용

### 2.5 투명도 / 블렌딩
- [ ] Alpha 합성 (Porter-Duff)
- [ ] 16종 블렌딩 모드 (Normal, Multiply, Screen, Overlay, ...)
- [ ] Transparency Group (Isolated, Knockout)
- [ ] Soft Mask
- [ ] Opacity (ca, CA)

### 2.6 셰이딩/그래디언트
- [ ] Axial (선형) 그래디언트
- [ ] Radial (원형) 그래디언트
- [ ] Function-based 셰이딩
- [ ] Free-form Gouraud 메시
- [ ] Coons/Tensor-product 패치 메시

### 2.7 패턴
- [ ] Tiling Pattern (colored / uncolored)
- [ ] Shading Pattern

### 2.8 출력 포맷
- [ ] PNG 출력
- [ ] JPEG 출력
- [ ] SVG 출력 (SVG Device)
- [ ] Raw Pixmap (RGBA, Gray)

### 2.T 테스트 요구사항

**Positive Tests:**
- [ ] 단순 도형 PDF (사각형, 원, 선) → 렌더링 결과 픽셀 검증
- [ ] 알려진 색상 fill → 출력 Pixmap 특정 좌표의 RGBA 값 확인
- [ ] 각 Line Cap/Join 스타일 → 레퍼런스 이미지와 비교
- [ ] Dash Pattern 적용 → 점선 렌더링 확인
- [ ] 텍스트 렌더링 → 글리프 위치/크기 정확성 (바운딩 박스 비교)
- [ ] CJK 텍스트 렌더링 → 글리프 표시 확인
- [ ] JPEG/PNG 이미지가 포함된 PDF → 이미지 정확히 배치
- [ ] 이미지 회전/스케일 (어핀 변환) → 결과 검증
- [ ] Alpha 합성 → 반투명 객체 겹침 색상 계산 검증
- [ ] 각 블렌딩 모드 (Multiply, Screen 등) → 알려진 입력/출력 색상 비교
- [ ] Axial/Radial 그래디언트 → 시작/끝 색상 검증
- [ ] Tiling Pattern → 패턴 반복 확인
- [ ] Display List 기록 → 재생 결과가 직접 렌더링과 동일
- [ ] PNG/JPEG 출력 → 파일 생성 확인, 이미지 뷰어로 열림

**Negative Tests:**
- [ ] 0x0 크기 페이지 → 에러 (빈 Pixmap 아님)
- [ ] 음수 크기 페이지 → 에러
- [ ] 매우 큰 페이지 (100000x100000) → OOM 대신 에러 또는 제한
- [ ] 잘못된 블렌딩 모드 이름 → 무시하고 Normal 폴백
- [ ] 깨진 이미지 데이터 → 해당 이미지만 스킵, 나머지 정상 렌더링
- [ ] 재귀적 Form XObject → 무한루프 방지
- [ ] 지원하지 않는 셰이딩 타입 → 스킵 (크래시 아님)

### 2.E 완료 확인 (직접 실행)
```bash
# PDF → PNG 렌더링
cargo run --example render -- your.pdf --page 1 --dpi 150 -o page1.png

# 전체 페이지 렌더링
cargo run --example render -- your.pdf --all --dpi 72 --out-dir ./rendered/

# 기대 결과: rendered/page_001.png, page_002.png, ... 생성
# 이미지 뷰어에서 열어 원본 PDF와 비교

# SVG 출력
cargo run --example render -- your.pdf --page 1 --format svg -o page1.svg

cargo test -p justpdf-render
```

---

## Phase 3: 텍스트 추출

> 목표: PDF에서 텍스트를 정확하게 추출하고 검색할 수 있는 상태

### 3.0 Phase 1에서 이월된 항목
- [ ] CJK 폰트 (사전 정의 CMap) → 한글/중국어/일본어 텍스트

### 3.1 Text Device
- [ ] 문자 단위 추출 (위치, 크기, 폰트, 색상, Unicode)
- [ ] ToUnicode CMap 기반 유니코드 변환
- [ ] CID → GID → Unicode 폴백
- [ ] ActualText 처리
- [ ] 리거처 확장

### 3.2 구조화 텍스트
- [ ] 문자 → 단어 그룹핑 (공백 추론)
- [ ] 단어 → 라인 그룹핑
- [ ] 라인 → 블록 그룹핑
- [ ] 읽기 순서 결정
- [ ] 다단(column) 레이아웃 감지

### 3.3 고급 텍스트 분석
- [ ] 표(table) 감지 및 추출
- [ ] 문단(paragraph) 감지
- [ ] 페이지 세그먼테이션
- [ ] 하이픈 제거 (dehyphenation)

### 3.4 텍스트 검색
- [ ] 정확한 문자열 검색
- [ ] 대소문자 무시 검색
- [ ] 정규식 검색
- [ ] 검색 결과 위치 (quad 좌표) 반환

### 3.5 텍스트 출력 포맷
- [ ] Plain text
- [ ] HTML (서식 유지)
- [ ] JSON (구조화 데이터)
- [ ] Markdown

### 3.T 테스트 요구사항

**Positive Tests:**
- [ ] 알려진 텍스트 내용의 PDF → 추출 결과 문자열 일치
- [ ] ToUnicode CMap 있는 PDF → 유니코드 정확히 변환
- [ ] CJK 텍스트 PDF → 한글/중국어/일본어 추출 확인
- [ ] 리거처 (fi, fl) → 개별 문자로 확장
- [ ] 다단 레이아웃 PDF → 올바른 읽기 순서 (왼쪽 단 → 오른쪽 단)
- [ ] 표가 포함된 PDF → 행/열 구조 추출, 셀 내용 일치
- [ ] 하이픈으로 분리된 단어 → 결합 확인 (dehyphenation)
- [ ] 텍스트 검색 "keyword" → 페이지 번호 + quad 좌표 반환
- [ ] 대소문자 무시 검색 → 매칭 확인
- [ ] 정규식 검색 `\d{3}-\d{4}` → 전화번호 패턴 매칭
- [ ] Plain text / HTML / JSON / Markdown 출력 → 각 포맷 유효성

**Negative Tests:**
- [ ] 이미지만 있는 PDF (스캔) → 빈 텍스트 반환 (에러 아님)
- [ ] ToUnicode 없고 커스텀 인코딩 → 가능한 만큼 추출, 나머지 U+FFFD
- [ ] 존재하지 않는 검색어 → 빈 결과 (에러 아님)
- [ ] 빈 페이지 → 빈 텍스트 (에러 아님)
- [ ] 잘못된 정규식 패턴 → 컴파일 에러 반환

### 3.E 완료 확인 (직접 실행)
```bash
# 텍스트 추출
cargo run --example extract_text -- your.pdf

# 기대 결과: PDF 전체 텍스트가 stdout에 출력

# 특정 페이지 텍스트
cargo run --example extract_text -- your.pdf --page 3

# 구조화 텍스트 (JSON)
cargo run --example extract_text -- your.pdf --format json -o text.json

# 표 추출
cargo run --example extract_tables -- your.pdf --page 1

# 기대 결과:
# Table 1 (4 rows x 3 cols):
# | Name   | Age | City   |
# | Alice  | 30  | Seoul  |
# | Bob    | 25  | Busan  |
# ...

# 텍스트 검색
cargo run --example search_text -- your.pdf "검색어"

# 기대 결과:
# Found 3 matches:
# Page 1: (120.5, 340.2, 180.3, 352.8) "검색어"
# Page 5: (72.0, 500.1, 132.0, 512.7) "검색어"
# ...

cargo test -p justpdf-text
```

---

## Phase 4: PDF 생성/수정

> 목표: PDF를 생성하고 기존 PDF를 수정하여 저장

### 4.1 PDF Writer 기본
- [ ] PDF 헤더 쓰기
- [ ] 객체 직렬화 (모든 PDF 타입)
- [ ] xref 테이블 생성
- [ ] trailer 생성
- [ ] 스트림 압축 (FlateDecode)
- [ ] 문서 저장 (새 파일)

### 4.2 페이지 생성
- [ ] 빈 페이지 생성 (크기 지정)
- [ ] 컨텐츠 스트림 빌더 (경로, 텍스트, 이미지 추가)
- [ ] 리소스 딕셔너리 자동 관리
- [ ] 페이지 삽입/삭제/재배열

### 4.3 텍스트 쓰기
- [ ] 폰트 임베딩 (TrueType, OpenType)
- [ ] 폰트 서브세팅 (사용 글리프만 포함)
- [ ] ToUnicode CMap 생성
- [ ] CJK 텍스트 쓰기
- [ ] 텍스트 레이아웃 (줄바꿈, 정렬)

### 4.4 이미지 임베딩
- [ ] JPEG 임베딩 (passthrough, 재인코딩 없이)
- [ ] PNG → FlateDecode + Predictor 변환
- [ ] 이미지 마스크 / 투명도
- [ ] 인라인 이미지

### 4.5 문서 수정
- [ ] 기존 PDF 수정 후 저장
- [ ] 증분 저장 (Incremental Save)
- [ ] 미사용 객체 정리 (Garbage Collection)
- [ ] 객체 스트림 (Object Streams) 생성
- [ ] 구문 정리/최적화 (Clean)

### 4.6 페이지 병합
- [ ] 여러 PDF에서 페이지 추출/병합
- [ ] 객체 이식 (Graft) - 중복 방지
- [ ] 리소스 충돌 해결

### 4.7 메타데이터
- [ ] Document Info 딕셔너리 (Title, Author, Subject, Keywords, ...)
- [ ] XMP 메타데이터 읽기/쓰기

### 4.T 테스트 요구사항

**Positive Tests:**
- [ ] 빈 PDF 생성 → 유효한 PDF (Adobe Reader/브라우저에서 열림)
- [ ] 생성한 PDF를 justpdf로 다시 파싱 → 왕복(roundtrip) 검증
- [ ] 텍스트 쓰기 → 추출했을 때 동일한 문자열
- [ ] 한글/CJK 텍스트 쓰기 → 추출 결과 일치
- [ ] TrueType 폰트 임베딩 → 서브세팅 후 파일 크기 감소 확인
- [ ] JPEG 이미지 임베딩 → passthrough (재인코딩 없이 바이트 동일)
- [ ] PNG 이미지 임베딩 → 투명도 유지
- [ ] 페이지 삽입/삭제/재배열 → 페이지 수/순서 확인
- [ ] 두 PDF 병합 → 페이지 수 = A + B
- [ ] 증분 저장 → 원본 데이터 유지 + 새 데이터 append
- [ ] garbage collection → 미사용 객체 제거, 파일 크기 감소
- [ ] 메타데이터 설정 (Title, Author) → 다시 읽었을 때 일치

**Negative Tests:**
- [ ] 음수 페이지 크기 → 에러
- [ ] 빈 문자열 폰트 이름 → 에러
- [ ] 존재하지 않는 폰트 파일 경로 → 에러
- [ ] 깨진 이미지 파일 임베딩 시도 → 에러
- [ ] 읽기 전용 경로에 저장 시도 → I/O 에러
- [ ] 페이지 삭제 후 범위 초과 페이지 접근 → 에러
- [ ] 순환 참조 생성 시도 → 감지/방지

### 4.E 완료 확인 (직접 실행)
```bash
# 빈 PDF 생성
cargo run --example create_pdf -- -o hello.pdf

# 기대 결과: hello.pdf 생성, 브라우저/PDF 뷰어에서 "Hello, World!" 표시

# 텍스트 + 이미지 포함 PDF 생성
cargo run --example create_pdf -- --text "안녕하세요" --image photo.jpg -o output.pdf

# PDF 병합
cargo run --example merge_pdf -- a.pdf b.pdf -o merged.pdf

# 기대 결과: merged.pdf = a.pdf 페이지들 + b.pdf 페이지들

# 페이지 추출
cargo run --example split_pdf -- your.pdf --pages 2-5 -o extracted.pdf

# 메타데이터 설정
cargo run --example set_metadata -- your.pdf --title "My Doc" --author "Me" -o updated.pdf

cargo test -p justpdf-writer
```

---

## Phase 5: 어노테이션 & 폼

> 목표: PDF 어노테이션과 대화형 폼을 완전히 지원

### 5.1 어노테이션 읽기
- [ ] 어노테이션 파싱 (전체 28종 타입)
- [ ] 어노테이션 속성 읽기 (Rect, Color, Border, Flags, Contents, ...)
- [ ] Appearance Stream 렌더링
- [ ] Popup 연결

### 5.2 어노테이션 생성/수정
- [ ] 마크업 어노테이션 생성 (Highlight, Underline, StrikeOut, Squiggly)
- [ ] 도형 어노테이션 (Line, Square, Circle, Polygon, PolyLine, Ink)
- [ ] 텍스트 어노테이션 (Text, FreeText, Stamp, Caret)
- [ ] 링크 어노테이션 (Link)
- [ ] 파일 첨부 (FileAttachment)
- [ ] Appearance Stream 자동 생성
- [ ] 어노테이션 삭제

### 5.3 폼 필드
- [ ] AcroForm 파싱
- [ ] 필드 타입: Text, Checkbox, RadioButton, ComboBox, ListBox, Button
- [ ] 필드 값 읽기/쓰기
- [ ] 필드 속성 (ReadOnly, Required, 등)
- [ ] 필드 외관 생성
- [ ] 폼 Flatten (정적 콘텐츠로 변환)

### 5.4 Redaction
- [ ] Redact 어노테이션 생성
- [ ] Redaction 적용 (텍스트/이미지/벡터 제거)
- [ ] 옵션: black box, 이미지 제거 방식, 라인아트 처리

### 5.T 테스트 요구사항

**Positive Tests:**
- [ ] 어노테이션 있는 PDF → 각 어노테이션 타입/속성 정확히 파싱
- [ ] Highlight 어노테이션 추가 → 저장 → 다시 열었을 때 존재 확인
- [ ] Ink 어노테이션 (자유 그리기) → ink list 좌표 왕복 검증
- [ ] Line 어노테이션 → line ending style, leader 속성 확인
- [ ] 어노테이션 삭제 → 저장 후 해당 어노테이션 없음 확인
- [ ] Appearance Stream 자동 생성 → 렌더링 결과 확인
- [ ] AcroForm PDF → 모든 필드 타입/값 파싱
- [ ] Text 필드 값 변경 → 저장 → 다시 읽었을 때 변경된 값
- [ ] Checkbox 토글 → 상태 변경 확인
- [ ] ComboBox 선택 변경 → 값 확인
- [ ] 폼 Flatten → 필드 사라지고 텍스트만 남음
- [ ] Redaction 적용 → 해당 영역 텍스트 추출 시 빈 결과

**Negative Tests:**
- [ ] 어노테이션 없는 PDF → 빈 리스트 반환 (에러 아님)
- [ ] 잘못된 어노테이션 타입 문자열 → Unknown 처리
- [ ] 필수 속성 누락된 어노테이션 → 스킵 또는 기본값
- [ ] AcroForm 없는 PDF에서 필드 조회 → 빈 결과
- [ ] ReadOnly 필드 값 변경 시도 → 에러
- [ ] 존재하지 않는 어노테이션 삭제 시도 → 에러

### 5.E 완료 확인 (직접 실행)
```bash
# 어노테이션 목록 출력
cargo run --example list_annots -- your.pdf

# 기대 결과:
# Page 1: 3 annotations
#   [0] Highlight color=(1,1,0) rect=(100,200,300,220)
#   [1] Text contents="Note here" rect=(400,500,420,520)
#   [2] Link uri="https://example.com"

# 하이라이트 추가
cargo run --example add_highlight -- your.pdf --page 1 --rect 100,200,300,220 --color yellow -o highlighted.pdf

# 폼 필드 목록
cargo run --example list_fields -- form.pdf

# 기대 결과:
# Field "name" (Text): "John Doe"
# Field "agree" (Checkbox): checked
# Field "country" (ComboBox): "Korea"

# 폼 필드 값 변경
cargo run --example fill_form -- form.pdf --field name="Jane" --field agree=false -o filled.pdf

# 폼 Flatten
cargo run --example flatten_form -- form.pdf -o flattened.pdf

cargo test -p justpdf-annot
```

---

## Phase 6: 보안

> 목표: PDF 암호화/복호화, 디지털 서명 지원

### 6.0 Phase 1에서 이월된 항목
- [ ] Crypt 스트림 필터

### 6.1 복호화 (읽기)
- [ ] RC4-40, RC4-128
- [ ] AES-128
- [ ] AES-256
- [ ] Owner/User 비밀번호 인증
- [ ] 권한 플래그 확인 (Print, Copy, Modify, ...)
- [ ] 암호화된 스트림/문자열 투명 복호화

### 6.2 암호화 (쓰기)
- [ ] RC4-128 암호화
- [ ] AES-128, AES-256 암호화
- [ ] Owner/User 비밀번호 설정
- [ ] 권한 플래그 설정
- [ ] 메타데이터 암호화 옵션

### 6.3 디지털 서명
- [ ] 서명 필드 감지
- [ ] PKCS#7 서명 검증
- [ ] 인증서 체인 검증
- [ ] 다이제스트 검증 (SHA-256, SHA-384, SHA-512)
- [ ] 서명 후 변경 감지 (Incremental change detection)
- [ ] PDF 서명 생성
- [ ] 서명 외관 생성
- [ ] 타임스탬프 서명

### 6.T 테스트 요구사항

**Positive Tests:**
- [ ] RC4-128 암호화 PDF → user password로 복호화 성공
- [ ] AES-128 암호화 PDF → 복호화 후 텍스트 추출 일치
- [ ] AES-256 암호화 PDF → 복호화 성공
- [ ] owner password 인증 → 전체 권한
- [ ] user password 인증 → 제한된 권한 확인
- [ ] 권한 플래그 (Print, Copy, Modify) → 정확히 읽기
- [ ] PDF 암호화 → 다시 복호화 → 왕복 검증
- [ ] 암호화 PDF 생성 → Adobe Reader에서 비밀번호 입력 후 열림
- [ ] 서명된 PDF → 서명 검증 성공 (유효한 인증서)
- [ ] 서명 후 수정된 PDF → 변경 감지
- [ ] PDF에 서명 추가 → 검증 통과

**Negative Tests:**
- [ ] 틀린 비밀번호 → 인증 실패 에러
- [ ] 빈 비밀번호 (owner pwd 필요한 PDF) → 에러
- [ ] 암호화된 PDF를 비밀번호 없이 텍스트 추출 → 에러
- [ ] 자체 서명 인증서 → NotTrusted 에러 (옵션으로 허용 가능)
- [ ] 손상된 서명 데이터 → DigestFailure
- [ ] 서명 필드 없는 PDF에서 서명 검증 → NoSignatures
- [ ] 지원하지 않는 암호화 알고리즘 → 명확한 에러 메시지

### 6.E 완료 확인 (직접 실행)
```bash
# 암호화된 PDF 열기
cargo run --example decrypt_pdf -- encrypted.pdf --password "secret" -o decrypted.pdf

# 기대 결과: decrypted.pdf 생성, 정상적으로 열림

# PDF 암호화
cargo run --example encrypt_pdf -- your.pdf --user-password "read" --owner-password "admin" \
    --no-print --no-copy -o secured.pdf

# 기대 결과: secured.pdf는 "read" 입력해야 열리고, 인쇄/복사 불가

# 서명 검증
cargo run --example verify_signature -- signed.pdf

# 기대 결과:
# Signature 1: VALID
#   Signer: CN=John Doe, O=Example Corp
#   Date: 2025-01-15 10:30:00 UTC
#   Digest: SHA-256 OK
#   Certificate: Trusted
#   Modified after signing: No

# PDF 서명
cargo run --example sign_pdf -- your.pdf --cert my-cert.p12 --password "certpass" -o signed.pdf

cargo test -p justpdf-crypto
cargo test -p justpdf-sign
```

---

## Phase 7: 고급 PDF 기능

> 목표: 프로덕션 수준의 PDF 처리를 위한 고급 기능

### 7.0 Phase 1에서 이월된 항목
- [ ] Properties (Optional Content) 리소스 로딩

### 7.1 북마크/아웃라인
- [ ] 아웃라인 트리 읽기 (제목, 목적지, 스타일)
- [ ] 아웃라인 생성/수정/삭제
- [ ] Named Destination 해석

### 7.2 링크/액션
- [ ] URI 링크
- [ ] GoTo (내부 페이지 이동)
- [ ] GoToR (외부 파일)
- [ ] Named Action (NextPage, PrevPage, FirstPage, LastPage)
- [ ] Launch, JavaScript 액션

### 7.3 Optional Content (레이어)
- [ ] OCG (Optional Content Group) 파싱
- [ ] OCMD (Optional Content Membership Dictionary)
- [ ] 레이어 활성화/비활성화
- [ ] 레이어 설정 (Config)
- [ ] Usage (Print, View, Export)

### 7.4 Linearized PDF
- [ ] Linearized PDF 판별
- [ ] 힌트 테이블 파싱
- [ ] Progressive loading (첫 페이지 우선 렌더링)
- [ ] Linearized PDF 생성

### 7.5 ICC 색상 관리
- [ ] ICC 프로필 파싱
- [ ] ICC 기반 색 변환 (littlecms Rust 포트 또는 자체 구현)
- [ ] Rendering Intent 적용
- [ ] Output Intent
- [ ] 오버프린트 시뮬레이션

### 7.6 고급 폰트
- [ ] CFF 폰트 파싱
- [ ] OpenType 레이아웃 (GSUB, GPOS) - text shaping
- [ ] Type3 폰트 렌더링
- [ ] Font descriptor 해석
- [ ] 폰트 복구 (손상/누락 폰트 대체)
- [ ] CID-keyed 폰트 완전 지원

### 7.7 페이지 라벨
- [ ] 페이지 라벨 읽기 (i, ii, ..., 1, 2, ...)
- [ ] 페이지 라벨 설정/삭제
- [ ] 스타일: Decimal, UpperRoman, LowerRoman, UpperAlpha, LowerAlpha

### 7.8 임베디드 파일
- [ ] File Specification 파싱
- [ ] 임베디드 파일 추출
- [ ] 임베디드 파일 추가
- [ ] 체크섬 검증

### 7.9 PDF 복구
- [ ] 손상된 xref 복구
- [ ] 누락 xref 재구축
- [ ] 깨진 스트림 복구
- [ ] 오류 허용 파싱 (tolerant parsing)

### 7.10 저널/Undo-Redo
- [ ] 작업 저널 기록
- [ ] Undo/Redo 지원
- [ ] 저널 직렬화/역직렬화

### 7.T 테스트 요구사항

**Positive Tests:**
- [ ] 북마크 트리 읽기 → 제목/페이지 번호 정확성
- [ ] 북마크 추가/삭제 → 저장 후 검증
- [ ] Named Destination → 정확한 페이지/좌표 해석
- [ ] URI 링크 → URL 추출
- [ ] GoTo 링크 → 대상 페이지 번호 확인
- [ ] OCG 레이어 목록 → 이름/상태 확인
- [ ] 레이어 비활성화 → 렌더링에서 해당 콘텐츠 제외
- [ ] Linearized PDF → 첫 페이지 빠른 로딩 확인
- [ ] ICC 프로필 → 색 변환 결과 레퍼런스와 비교
- [ ] 페이지 라벨 (i, ii, 1, 2...) → 올바른 라벨 반환
- [ ] 임베디드 파일 추출 → 원본과 바이트 동일
- [ ] 손상된 xref PDF → 복구 후 페이지 접근 성공
- [ ] Undo/Redo → 상태 정확히 복원

**Negative Tests:**
- [ ] 북마크 없는 PDF → 빈 트리 (에러 아님)
- [ ] 잘못된 Destination 참조 → 에러 또는 무시
- [ ] OCG 없는 PDF에서 레이어 조회 → 빈 리스트
- [ ] 유효하지 않은 ICC 프로필 → 에러 또는 폴백
- [ ] 복구 불가능한 수준의 손상 → 명확한 에러
- [ ] 페이지 라벨 없는 PDF → 기본 숫자 반환

### 7.E 완료 확인 (직접 실행)
```bash
# 북마크 트리 출력
cargo run --example list_bookmarks -- your.pdf

# 기대 결과:
# 1. Introduction ................. page 1
#   1.1 Background ............... page 3
#   1.2 Scope .................... page 5
# 2. Methods ..................... page 10
# ...

# 레이어 목록 및 토글
cargo run --example list_layers -- layered.pdf

# 기대 결과:
# Layer "Background" [ON]
# Layer "Watermark" [OFF]
# Layer "Annotations" [ON]

cargo run --example toggle_layer -- layered.pdf --disable "Watermark" -o no_watermark.pdf

# 임베디드 파일 추출
cargo run --example extract_files -- your.pdf --out-dir ./attachments/

# PDF 복구
cargo run --example repair_pdf -- damaged.pdf -o repaired.pdf

# 페이지 라벨 확인
cargo run --example page_labels -- your.pdf

# 기대 결과:
# Page 1: "i"
# Page 2: "ii"
# Page 3: "1"
# Page 4: "2"

cargo test -p justpdf-core --features advanced
```

---

## Phase 8: 성능 최적화

> 목표: MuPDF와 동등하거나 그 이상의 성능

### 8.1 파싱 최적화
- [ ] Memory-mapped I/O
- [ ] Lazy 객체 로딩 (필요할 때만 역직렬화)
- [ ] 객체 캐싱 (LRU)
- [ ] 병렬 페이지 파싱

### 8.2 렌더링 최적화
- [ ] SIMD 래스터화 (SSE2/AVX2/NEON)
- [ ] 멀티스레드 렌더링 (페이지 단위 / 밴드 단위)
- [ ] 글리프 캐시 최적화
- [ ] Display List 최적화 (중복 제거)
- [ ] 타일 기반 렌더링

### 8.3 메모리 최적화
- [ ] Arena allocator (파싱용)
- [ ] 스트림 디코딩 zero-copy
- [ ] 대용량 PDF (10,000+ 페이지) 지원
- [ ] 메모리 사용량 프로파일링

### 8.4 벤치마크
- [ ] MuPDF vs justpdf 렌더링 속도 비교
- [ ] MuPDF vs justpdf 텍스트 추출 속도 비교
- [ ] MuPDF vs justpdf 메모리 사용량 비교
- [ ] 대규모 PDF 코퍼스 회귀 테스트

### 8.T 테스트 요구사항

**Positive Tests:**
- [ ] 1000+ 페이지 PDF 파싱 → 메모리 사용량 합리적 범위 내
- [ ] 멀티스레드 렌더링 → 싱글스레드 대비 속도 향상 확인
- [ ] mmap 모드 → 일반 모드 대비 메모리 사용 감소
- [ ] lazy loading → 첫 페이지 접근 시간 < 전체 로딩 시간
- [ ] 글리프 캐시 → 동일 폰트 반복 렌더링 속도 향상
- [ ] criterion 벤치마크 → 회귀 없음 (이전 결과 대비)

**Negative Tests:**
- [ ] 10,000+ 페이지 PDF → OOM 없이 처리 (또는 명확한 제한 에러)
- [ ] 스레드 수 0 → 에러 또는 기본값 폴백
- [ ] 손상된 mmap 파일 → 안전한 에러 처리

### 8.E 완료 확인 (직접 실행)
```bash
# 벤치마크 실행
cargo bench

# 기대 결과:
# parse_pdf/small.pdf    time: [1.2 ms 1.3 ms 1.4 ms]
# parse_pdf/large.pdf    time: [45 ms 47 ms 49 ms]
# render_page/page1      time: [12 ms 13 ms 14 ms]
# extract_text/10pages   time: [3.2 ms 3.4 ms 3.6 ms]

# 대용량 PDF 프로파일링
cargo run --example profile -- large.pdf

# 기대 결과:
# File: large.pdf (1,234 pages, 45.2 MB)
# Parse time: 120ms
# Memory usage: 28 MB (lazy), 180 MB (full load)
# Render page 1: 35ms (single-thread), 12ms (4 threads)
# Text extract all: 890ms

# MuPDF 비교 (mutool이 설치되어 있을 때)
cargo run --example compare_mupdf -- your.pdf

cargo test --release
```

---

## Phase 9: 확장 포맷 (선택)

> 목표: PDF 외 문서 포맷 지원 (MuPDF 패리티)

### 9.1 XPS
- [ ] XPS/OpenXPS 파서
- [ ] XPS 렌더링
- [ ] XPS → PDF 변환

### 9.2 EPUB
- [ ] EPUB 컨테이너 파싱 (ZIP + OPF)
- [ ] HTML/CSS 레이아웃 엔진
- [ ] Reflowable 문서 지원
- [ ] EPUB → PDF 변환

### 9.3 SVG
- [ ] SVG 파싱
- [ ] SVG 렌더링
- [ ] PDF → SVG 변환
- [ ] SVG → PDF 변환

### 9.4 Office 포맷
- [ ] DOCX 파싱/렌더링
- [ ] XLSX 파싱/렌더링
- [ ] PPTX 파싱/렌더링

### 9.5 기타
- [ ] CBZ/CBT (Comic Book Archive)
- [ ] MOBI/FB2 eBook
- [ ] Plain Text → PDF

### 9.T 테스트 요구사항

**Positive Tests:**
- [ ] XPS 파일 → 페이지 수/크기 파싱
- [ ] XPS → PNG 렌더링 → 글리프/도형 표시
- [ ] EPUB → 챕터 목록 파싱, 텍스트 추출
- [ ] EPUB → PDF 변환 → 유효한 PDF 생성
- [ ] SVG 파싱 → 요소(path, text, image) 추출
- [ ] SVG → PNG 렌더링
- [ ] PDF → SVG 변환 → 브라우저에서 열림
- [ ] DOCX → 텍스트 추출
- [ ] CBZ → 이미지 목록 / 페이지 렌더링

**Negative Tests:**
- [ ] 손상된 EPUB (ZIP 깨짐) → 에러
- [ ] 잘못된 SVG (XML 에러) → 파싱 에러
- [ ] 지원하지 않는 포맷 확장자 → 명확한 에러
- [ ] DRM 보호된 EPUB → 지원 불가 에러

### 9.E 완료 확인 (직접 실행)
```bash
# XPS 렌더링
cargo run --example render -- document.xps --page 1 -o xps_page1.png

# EPUB → PDF
cargo run --example convert -- book.epub -o book.pdf

# SVG 렌더링
cargo run --example render -- image.svg -o rendered.png

# PDF → SVG
cargo run --example convert -- your.pdf --format svg --page 1 -o page1.svg

# DOCX 텍스트 추출
cargo run --example extract_text -- document.docx

cargo test -p justpdf --features "xps epub svg office"
```

---

## Phase 10: 특수 기능 (선택)

### 10.1 OCR
- [ ] Tesseract 연동 (또는 Rust OCR 엔진)
- [ ] 스캔 PDF → 검색 가능 PDF 변환

### 10.2 바코드
- [ ] QR Code 생성/인식
- [ ] 1D 바코드 (Code128, EAN13, UPC-A, ...)
- [ ] 2D 바코드 (DataMatrix, PDF417, Aztec, ...)

### 10.3 ZUGFeRD
- [ ] 전자 인보이스 프로필 감지
- [ ] XML 추출/생성

### 10.4 BiDi 텍스트
- [ ] Unicode Bidirectional Algorithm 구현
- [ ] RTL/LTR 혼합 텍스트 처리

### 10.5 Deskew
- [ ] 스캔 이미지 기울기 감지
- [ ] 자동 보정

### 10.T 테스트 요구사항

**Positive Tests:**
- [ ] 스캔 PDF → OCR 텍스트 레이어 추가 → 텍스트 검색 가능
- [ ] QR Code 생성 → 디코딩 → 원본 데이터 일치
- [ ] 1D 바코드 (EAN13 등) → 생성 → 디코딩 왕복
- [ ] ZUGFeRD PDF → 프로필/XML 추출
- [ ] RTL 텍스트 (아랍어) → 올바른 방향 렌더링/추출
- [ ] 기울어진 스캔 이미지 → deskew → 보정 각도 확인

**Negative Tests:**
- [ ] Tesseract 미설치 시 OCR → 명확한 에러
- [ ] 빈 이미지에서 바코드 디코딩 → 빈 결과 (에러 아님)
- [ ] ZUGFeRD 아닌 PDF에서 프로필 조회 → NotZugferd
- [ ] 완전 검은 이미지 deskew → 각도 0 반환 (에러 아님)

### 10.E 완료 확인 (직접 실행)
```bash
# OCR
cargo run --example ocr_pdf -- scanned.pdf -o searchable.pdf
# 기대 결과: searchable.pdf에서 텍스트 검색 가능

# 바코드 생성
cargo run --example barcode -- --type qr --data "https://example.com" -o qr.png

# 바코드 인식
cargo run --example barcode -- --decode page_with_qr.pdf
# 기대 결과: "https://example.com"

# ZUGFeRD
cargo run --example zugferd -- invoice.pdf
# 기대 결과:
# Profile: XRechnung
# XML: (invoice XML 출력)

cargo test -p justpdf --features "ocr barcode zugferd bidi deskew"
```

---

## Phase 11: API & 생태계

### 11.1 Public API
- [ ] 고수준 API 설계 (Document, Page, TextExtractor, Renderer, Writer)
- [ ] Builder 패턴 기반 PDF 생성 API
- [ ] Iterator 기반 페이지/어노테이션 순회
- [ ] async 지원 (tokio 호환)
- [ ] Error 타입 설계 (thiserror)

### 11.2 CLI 도구
- [ ] `justpdf render` - 페이지 렌더링 (PNG/JPEG)
- [ ] `justpdf text` - 텍스트 추출
- [ ] `justpdf info` - PDF 정보 표시
- [ ] `justpdf merge` - PDF 병합
- [ ] `justpdf split` - 페이지 분리
- [ ] `justpdf encrypt` / `decrypt` - 암호화 관리
- [ ] `justpdf sign` - 디지털 서명
- [ ] `justpdf clean` - 최적화/복구
- [ ] `justpdf convert` - 포맷 변환

### 11.3 Language Bindings
- [ ] C API (FFI)
- [ ] Python 바인딩 (PyO3)
- [ ] Node.js 바인딩 (napi-rs)
- [ ] WASM 빌드 (wasm-bindgen)

### 11.4 문서화
- [ ] API 문서 (rustdoc)
- [ ] 사용 가이드 (mdbook)
- [ ] 예제 코드 (examples/)
- [ ] CHANGELOG 유지

### 11.T 테스트 요구사항

**Positive Tests:**
- [ ] 고수준 API → Document::open → page(0) → render → save 체인 동작
- [ ] Builder API → PdfWriter::new().add_page().add_text().save() 동작
- [ ] Iterator → doc.pages().map(|p| p.text()).collect() 동작
- [ ] async API → tokio::spawn에서 Document::open_async 동작
- [ ] CLI `justpdf render your.pdf -o out.png` → 종료코드 0 + 파일 생성
- [ ] CLI `justpdf text your.pdf` → stdout에 텍스트 출력
- [ ] CLI `justpdf info your.pdf` → PDF 정보 출력
- [ ] CLI `justpdf merge a.pdf b.pdf -o merged.pdf` → 동작
- [ ] Python: `import justpdf; doc = justpdf.open("your.pdf")` 동작
- [ ] WASM: 브라우저에서 PDF 렌더링 동작
- [ ] `cargo doc --no-deps` → 경고 없이 문서 빌드

**Negative Tests:**
- [ ] CLI 인자 없이 실행 → help 메시지 (크래시 아님)
- [ ] CLI 잘못된 서브커맨드 → 에러 메시지 + 종료코드 1
- [ ] CLI 존재하지 않는 입력 파일 → 에러 메시지
- [ ] Python에서 잘못된 경로 → Python exception (세그폴트 아님)
- [ ] WASM에서 메모리 초과 → JS 에러 (크래시 아님)

### 11.E 완료 확인 (직접 실행)
```bash
# CLI 도구
justpdf info your.pdf
# 기대 결과:
# File: your.pdf
# Version: 1.7
# Pages: 12
# Title: "My Document"
# Author: "John Doe"
# Encrypted: No
# File size: 1.2 MB

justpdf render your.pdf --page 1 --dpi 150 -o page.png
justpdf text your.pdf > extracted.txt
justpdf merge a.pdf b.pdf -o merged.pdf

# Python
python -c "
import justpdf
doc = justpdf.open('your.pdf')
print(f'Pages: {doc.page_count}')
page = doc[0]
text = page.get_text()
print(text[:200])
page.render(dpi=150).save('page.png')
"

# WASM (브라우저 개발서버)
cargo run --example wasm_demo

# API 문서
cargo doc --open

cargo test --all
```

---

## 의존성 후보 (Rust Crates)

| 영역 | Crate | 용도 |
|------|-------|------|
| 압축 | `flate2`, `miniz_oxide` | Deflate/Inflate |
| 압축 | `weezl` | LZW |
| 이미지 | `image`, `jpeg-decoder`, `png` | 이미지 디코딩 |
| 이미지 | `jpeg2k` 또는 `openjpeg-sys` | JPEG2000 |
| 이미지 | `jbig2dec` (바인딩 또는 포트) | JBIG2 |
| 폰트 | `ab_glyph`, `owned_ttf_parser` | 폰트 파싱/글리프 |
| 폰트 | `rustybuzz` | Text shaping (HarfBuzz 포트) |
| 폰트 | `subsetter` | 폰트 서브세팅 |
| 암호 | `aes`, `rc4` (RustCrypto) | 대칭 암호 |
| 암호 | `md-5`, `sha2` (RustCrypto) | 해시 |
| 암호 | `pkcs7`, `x509-cert` | 디지털 서명 |
| 색상 | `lcms2` 또는 자체 구현 | ICC 색상 관리 |
| 래스터 | `tiny-skia` | 2D 래스터화 (참고/대안) |
| 인코딩 | `encoding_rs` | 문자 인코딩 변환 |
| CLI | `clap` | 커맨드라인 파서 |
| 비동기 | `tokio` | async I/O |
| 에러 | `thiserror`, `anyhow` | 에러 처리 |
| 테스트 | `criterion` | 벤치마크 |
| FFI | `pyo3`, `napi-rs`, `wasm-bindgen` | 바인딩 |

---

## 마일스톤 요약

| 마일스톤 | 목표 | 핵심 산출물 |
|----------|------|-------------|
| **M0** | 기반 구축 | PDF 파서, 객체 모델, xref, 기본 필터 |
| **M1** | PDF 읽기 | 컨텐츠 스트림 해석, 폰트, 이미지, 색공간 |
| **M2** | 렌더링 | 페이지 → PNG 변환 가능 |
| **M3** | 텍스트 추출 | 텍스트/표 추출, 검색 |
| **M4** | PDF 생성 | PDF 쓰기, 페이지 생성, 병합, 증분 저장 |
| **M5** | 어노테이션 & 폼 | 전체 어노테이션, AcroForm, Redaction |
| **M6** | 보안 | 암호화/복호화, 디지털 서명 |
| **M7** | 고급 기능 | 레이어, Linearized, ICC, 북마크, 복구 |
| **M8** | 성능 | SIMD, 멀티스레드, 메모리 최적화 |
| **M9** | 확장 포맷 | XPS, EPUB, SVG, Office |
| **M10** | 특수 기능 | OCR, 바코드, ZUGFeRD |
| **M11** | 생태계 | CLI, Python/Node/WASM 바인딩, 문서화 |
