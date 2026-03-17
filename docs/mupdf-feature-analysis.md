# MuPDF 기능 전체 분석

> MuPDF 소스코드 기반 분석 (mupdf-reference submodule)
> justpdf 프로젝트를 위한 기능 레퍼런스

---

## 1. 지원 입력 포맷

### 문서 포맷
| 포맷 | 확장자 | 설명 |
|------|--------|------|
| PDF | `.pdf` | PDF 1.0 ~ 2.0 |
| XPS | `.xps`, `.oxps` | XML Paper Specification / OpenXPS |
| EPUB | `.epub` | Electronic Publication (ZIP 기반) |
| MOBI | `.mobi`, `.prc`, `.pdb` | Mobipocket eBook |
| FB2 | `.fb2` | FictionBook 2.0 |
| HTML | `.html`, `.htm`, `.xhtml` | HTML5 / XHTML |
| TXT | `.txt`, `.text`, `.log` | Plain text |
| SVG | `.svg` | Scalable Vector Graphics |
| CBZ/CBT/CBR | `.cbz`, `.cbt`, `.cbr` | Comic Book Archive (ZIP/TAR/RAR) |
| Office | `.docx`, `.xlsx`, `.pptx`, `.hwpx` | MS Office / Hancom |

### 이미지 포맷 (입력)
| 포맷 | 설명 |
|------|------|
| JPEG | DCT 압축 이미지 |
| PNG | Portable Network Graphics |
| TIFF | Tagged Image File Format |
| BMP | Bitmap |
| GIF | Graphics Interchange Format |
| PSD | Adobe Photoshop |
| PNM/PBM/PGM/PPM/PAM | Portable Anymap 계열 |
| JBIG2 | 이진 이미지 압축 |
| JPEG2000 (JP2/JPX/J2K) | Wavelet 기반 이미지 압축 |
| JPEG-XR (JXR/HDP/WDP) | Microsoft HD Photo |

---

## 2. 지원 출력 포맷

| 포맷 | 파일 | 설명 |
|------|------|------|
| PDF | `output-pdfocr.c` | OCR 텍스트 레이어 포함 PDF |
| CBZ | `output-cbz.c` | Comic Book ZIP |
| SVG | `output-svg.c` | Scalable Vector Graphics |
| PNG | `output-png.c` | 래스터 이미지 |
| JPEG | `output-jpeg.c` | 래스터 이미지 |
| PNM | `output-pnm.c` | Portable Anymap |
| PSD | `output-psd.c` | Adobe Photoshop |
| PCL | `output-pcl.c` | Printer Command Language |
| PCLM | `output-pclm.c` | PCL for Mobile |
| PWG | `output-pwg.c` | Printer Working Group Raster |
| PostScript | `output-ps.c` | PostScript |
| CSV | `output-csv.c` | 텍스트 추출 (표 형식) |
| DOCX | `output-docx.c` | Microsoft Word |

---

## 3. PDF 핵심 기능

### 3.1 PDF 객체 모델
- **기본 타입**: Null, Bool, Int, Real, Name, String, Array, Dict, Stream, Indirect Reference
- **객체 생성/조회/수정/삭제** 전체 CRUD
- **Cross-reference (xref) 테이블** 관리
  - 일반 xref, 증분(incremental) xref
  - xref repair (손상 복구)
  - Linearized PDF 지원
- **객체 스트림** (Object Streams) 지원
- **Deep copy**, 비교(compare), 마킹(marking)

### 3.2 어노테이션 (Annotations)

**지원 어노테이션 타입 (28종)**:
| 타입 | 설명 |
|------|------|
| Text | 텍스트 노트 (팝업) |
| Link | 하이퍼링크 |
| FreeText | 자유 텍스트 (콜아웃 포함) |
| Line | 직선 (리더선, 캡션 포함) |
| Square | 사각형 |
| Circle | 원/타원 |
| Polygon | 다각형 |
| PolyLine | 다중 선분 |
| Highlight | 텍스트 하이라이트 |
| Underline | 밑줄 |
| Squiggly | 물결 밑줄 |
| StrikeOut | 취소선 |
| Redact | 삭제(편집) 표시 |
| Stamp | 도장 (이미지 스탬프 포함) |
| Caret | 삽입 기호 |
| Ink | 자유 그리기 (잉크 리스트) |
| Popup | 팝업 윈도우 |
| FileAttachment | 파일 첨부 |
| Sound | 소리 |
| Movie | 동영상 |
| RichMedia | 리치 미디어 |
| Widget | 폼 필드 |
| Screen | 스크린 |
| PrinterMark | 인쇄 마크 |
| TrapNet | 트랩 네트워크 |
| Watermark | 워터마크 |
| 3D | 3D 콘텐츠 |
| Projection | 프로젝션 |

**어노테이션 속성**:
- Rect, Color, Interior Color, Opacity, Border (width/style/dash/effect)
- Line Ending Styles: None, Square, Circle, Diamond, OpenArrow, ClosedArrow, Butt, ROpenArrow, RClosedArrow, Slash
- Border Styles: Solid, Dashed, Beveled, Inset, Underline
- Border Effects: None, Cloudy
- Intent: Default, FreeText Callout/Typewriter, Line Arrow/Dimension, Polygon Cloud/Dimension, Stamp Image/Snapshot
- Flags: Invisible, Hidden, Print, NoZoom, NoRotate, NoView, ReadOnly, Locked, ToggleNoView, LockedContents
- Quad Points (텍스트 마크업), Ink List (잉크), Vertices (폴리곤)
- Callout Line, Line Leader/Extension/Offset, Caption
- Author, Contents, CreationDate, ModificationDate
- Default Appearance (폰트, 크기, 색상)
- Rich Contents, Rich Defaults
- File Specification (첨부파일)
- Appearance Stream 생성/합성

### 3.3 폼 필드 (Interactive Forms / AcroForms)

**위젯 타입**:
| 타입 | 설명 |
|------|------|
| Button | 푸시 버튼 |
| Checkbox | 체크박스 |
| RadioButton | 라디오 버튼 |
| Text | 텍스트 입력 |
| ComboBox | 콤보박스 (드롭다운) |
| ListBox | 리스트박스 |
| Signature | 서명 필드 |

**텍스트 필드 포맷**: None, Number, Special, Date, Time

**필드 플래그**:
- ReadOnly, Required, NoExport
- Text: Multiline, Password, FileSelect, DoNotSpellCheck, DoNotScroll, Comb, RichText
- Button: NoToggleToOff, Radio, Pushbutton, RadiosInUnison
- Choice: Combo, Edit, Sort, MultiSelect, DoNotSpellCheck, CommitOnSelChange

**폼 연산**:
- 필드 생성/조회/수정/삭제
- 폼 계산 (calculate), 리셋 (reset)
- Keystroke/Validate/Calculate/Format 이벤트
- Document 이벤트 (WillClose, WillSave, DidSave, WillPrint, DidPrint)
- Page 이벤트 (Open, Close)
- Annotation 이벤트 (Enter, Exit, Down, Up, Focus, Blur, PageOpen/Close/Visible/Invisible)
- Baking (폼을 정적 콘텐츠로 변환)

### 3.4 디지털 서명 (Digital Signatures)

**서명 기능**:
- PKCS#7 기반 서명/검증
- 서명 생성 (Signer 인터페이스)
- 서명 검증 (Verifier 인터페이스)
- 인증서 체인 검증
- 다이제스트 검증
- 서명자 정보 추출 (Distinguished Name)
- 서명 미리보기 (Display List / Pixmap)
- 서명 외관 커스터마이징 (Labels, DN, Date, TextName, GraphicName, Logo)
- 증분 변경 감지 (signing 이후 변경 여부)
- Byte Range 검증
- 필드 잠금 (Locked Fields)

**서명 에러 코드**: OK, NoSignatures, NoCertificate, DigestFailure, SelfSigned, SelfSignedInChain, NotTrusted, NotSigned, Unknown

### 3.5 암호화/보안 (Encryption)

**암호화 방식**:
| 방식 | 설명 |
|------|------|
| RC4-40 | 40비트 RC4 (PDF 1.1~1.3) |
| RC4-128 | 128비트 RC4 (PDF 1.4) |
| AES-128 | 128비트 AES (PDF 1.5) |
| AES-256 | 256비트 AES (PDF 1.7 ext3, 2.0) |

**권한 플래그**:
| 플래그 | 설명 |
|--------|------|
| Print | 인쇄 |
| Modify | 문서 수정 |
| Copy | 텍스트/이미지 복사 |
| Annotate | 어노테이션 추가/수정 |
| Form | 폼 필드 채우기 |
| Accessibility | 접근성 텍스트 추출 |
| Assemble | 문서 조합 (페이지 삽입/삭제) |
| PrintHQ | 고품질 인쇄 |

**암호화 연산**:
- 비밀번호 인증 (Owner/User password)
- 스트림/문자열 암호화/복호화
- 메타데이터 암호화 옵션

### 3.6 페이지 조작

**Page Box 타입**: MediaBox, CropBox, BleedBox, TrimBox, ArtBox

**페이지 연산**:
- 페이지 생성/삽입/삭제/범위 삭제
- 페이지 변환 (transform), 바운딩 박스
- 투명도 감지
- 분리색 (Separations) 추출
- 리소스/컨텐츠/그룹 접근
- 페이지 렌더링 (전체, 컨텐츠만, 어노테이션만, 위젯만)
- 페이지 필터링 (컨텐츠/어노테이션)
- 페이지 편집 (Redaction)
- 페이지 벡터화 (Vectorize)
- 페이지 클리핑
- 프레젠테이션 트랜지션
- 기본 색공간 로드/업데이트

**Redaction 옵션**:
- black_boxes: 검은 박스로 덮기
- image_method: None, Remove, Pixels, RemoveUnlessInvisible
- line_art: None, RemoveIfCovered, RemoveIfTouched
- text: Remove, None, RemoveInvisible

### 3.7 문서 레벨 기능

**북마크/아웃라인**: 로드, 반복자, 트리 구조 탐색

**레이어/Optional Content (OCG)**:
- 레이어 설정 (config) 관리
- 레이어 활성화/비활성화
- 레이어 UI 정보 (checkbox, radiobutton, label)
- 레이어 설정 저장

**페이지 라벨**: 읽기/설정/삭제, 스타일 (Decimal, UpperRoman, LowerRoman, UpperAlpha, LowerAlpha)

**메타데이터**: Title, Author, Subject, Keywords, Creator, Producer, CreationDate, ModDate 등

**저널/Undo-Redo**:
- 저널 활성화
- 작업 시작/종료/폐기
- Undo/Redo
- 저널 직렬화/역직렬화
- 저널 저장/로드

**Graft (페이지 이식)**:
- 다른 PDF에서 객체/페이지 가져오기
- Graft Map을 통한 효율적 복제

**문서 저장 옵션**:
| 옵션 | 설명 |
|------|------|
| incremental | 증분 저장 |
| pretty | 보기 좋은 형식 |
| ascii | ASCII 인코딩 |
| compress | 스트림 압축 |
| compress_images | 이미지 압축 |
| compress_fonts | 폰트 압축 |
| decompress | 스트림 해제 |
| garbage | 미사용 객체 제거 (0~4단계) |
| linear | Linearized PDF 생성 |
| clean | 구문 정리 |
| sanitize | 살균 |
| appearance | 외관 스트림 생성 |
| encrypt | 암호화 방식 설정 |
| snapshot | 스냅샷 저장 |
| preserve_metadata | 메타데이터 유지 |
| use_objstms | 객체 스트림 사용 |
| compression_effort | 압축 노력 수준 |
| labels | 페이지 라벨 포함 |

### 3.8 JavaScript 지원
- JS 엔진 활성화/비활성화
- 이벤트 처리 (init, result, validate, keystroke)
- 스크립트 실행
- 콘솔 접근

### 3.9 ZUGFeRD (전자 인보이스)
- 프로필 감지: Comfort, Basic, Extended, BasicWL, Minimum, XRechnung
- XML 데이터 추출

### 3.10 이미지 재작성 (Image Rewriter)

**서브샘플링 방식**: Average, Bicubic

**재압축 방식**: Never, Same, Lossless, JPEG, J2K, FAX

**옵션**:
- 컬러/그레이스케일/이진 이미지별 서브샘플링 임계값 및 타겟 DPI
- 재압축 품질 설정
- 크기 비교 (더 작을 때만 / 항상)

### 3.11 리컬러링 (Recoloring)
- 페이지 색상 변환 (Gray, RGB, CMYK)
- Output Intent 제거

### 3.12 문서 정리/복구 (Clean)
- 구조 처리: Drop, Keep
- 벡터화: Yes, No
- 파일 정리 (`pdf_clean_file`)
- 페이지 재배열 (`pdf_rearrange_pages`)
- 이미지/폰트 최적화

---

## 4. 렌더링 엔진 (Fitz Core)

### 4.1 디바이스 모델
| 디바이스 | 설명 |
|----------|------|
| Draw Device | 래스터화 (Pixmap으로 렌더링) |
| Display List Device | 명령 기록/재생 |
| Text Device | 구조화 텍스트 추출 |
| BBox Device | 바운딩 박스 계산 |
| Trace Device | 디버그 출력 |
| SVG Device | SVG 벡터 출력 |
| OCR Device | Tesseract OCR 통합 |
| XML Text Device | XML 텍스트 출력 |

### 4.2 경로(Path) 렌더링
- MoveTo, LineTo, CurveTo (3차 베지에), ClosePath
- Degenerate 곡선 처리
- Rect shorthand
- Fill (Even-Odd / Winding) 및 Stroke
- Line Cap: Butt, Round, Square, Triangle
- Line Join: Miter, Round, Bevel, MiterXPS
- Dash Pattern 지원
- Miter Limit

### 4.3 블렌딩 모드 (16종)
- Normal
- Multiply, Screen, Overlay
- Darken, Lighten
- ColorDodge, ColorBurn
- HardLight, SoftLight
- Difference, Exclusion
- Hue, Saturation, Color, Luminosity
- Isolated / Knockout 그룹

### 4.4 색공간 (Color Spaces)
| 색공간 | 설명 |
|--------|------|
| DeviceGray | 그레이스케일 |
| DeviceRGB | RGB |
| DeviceBGR | BGR (화면 순서) |
| DeviceCMYK | CMYK |
| Lab | CIE L*a*b |
| Indexed | 팔레트 기반 |
| Separation | 스팟 컬러 |
| DeviceN | 다중 컬러런트 |
| ICC Profile | ICC 프로필 기반 (모든 타입) |
| CalGray | 캘리브레이션 Gray |
| CalRGB | 캘리브레이션 RGB |

**색공간 기능**:
- 최대 32개 컬러런트
- ICC 프로필 색상 변환 (LCMS 연동)
- 렌더링 인텐트: Perceptual, RelativeColorimetric, Saturation, AbsoluteColorimetric
- Black Point Compensation
- 오버프린트 지원
- 기본 색공간 관리

### 4.5 폰트 시스템
**지원 폰트 타입**: TrueType, Type1, Type3, CFF, OpenType

**폰트 기능**:
- 폰트 임베딩/서브세팅
- Bold/Italic 합성
- Small-caps 변환
- Glyph 치환 및 폴백
- HarfBuzz 텍스트 쉐이핑
- CJK 폰트 지원 (CNS, GB, Japan, Korea)
- Noto 폰트 (100+ 스크립트)
- Base14 표준 PDF 폰트
- 리거처 처리/확장
- 글리프 캐싱
- FreeType 통합

**인코딩**: ISO-8859-1, ISO-8859-7, KOI8-U, KOI8-R, Windows-1250/1251/1252, MacRoman, MacExpert, AdobeStandard, WinAnsi, PDFDocEncoding

### 4.6 이미지 처리
- 디코딩: JPEG, PNG, TIFF, BMP, GIF, PSD, PNM, JBIG2, JPEG2000, JPEG-XR
- 인코딩: PNG, JPEG
- Pixmap 조작 (생성, 변환, 스케일링, 감마 보정, 인버트, 틴트)
- 이미지 마스크, 소프트 마스크
- 인라인 이미지

### 4.7 셰이딩/그래디언트
- Function-based (Type 1)
- Axial (Type 2, 선형)
- Radial (Type 3, 원형)
- Free-form Gouraud (Type 4)
- Lattice-form Gouraud (Type 5)
- Coons patch (Type 6)
- Tensor-product patch (Type 7)

---

## 5. 텍스트 추출

### 5.1 추출 모드/옵션
| 옵션 | 설명 |
|------|------|
| Preserve Ligatures | 리거처 유지 vs 확장 |
| Preserve Whitespace | 공백 유지 vs 정규화 |
| Extract Images | 텍스트 흐름 내 이미지 추출 |
| Inhibit Spaces | 자동 공백 삽입 억제 |
| Dehyphenate | 소프트 하이픈 처리 |
| Preserve Spans | 폰트/색상/크기별 스팬 분리 |
| Clip to MediaBox | 미디어박스 클리핑 |
| Collect Structure | 문서 구조 수집 |
| Collect Vectors | 벡터 그래픽 수집 |
| Page Segmentation | 페이지 세그먼테이션 |
| Table Detection | 표 감지/마킹 |
| CID/GID to Unicode | CID/GID 유니코드 변환 |

### 5.2 구조화 텍스트 (Structured Text)
- **블록**: 텍스트 블록, 이미지 블록
- **라인**: 방향(wmode), 바운딩 박스
- **문자**: 위치, 크기, 폰트, 색상, Unicode 코드포인트
- 텍스트 검색 (정규식 포함)
- 표 추출 (stext-table)
- 문단 감지 (stext-para)
- 분류 (stext-classify)
- 박서 (stext-boxer) - 레이아웃 분석

---

## 6. 암호화/압축

### 6.1 해시 함수
- MD5
- SHA-256, SHA-384, SHA-512

### 6.2 대칭 암호
- AES (128/192/256비트, CBC 모드)
- RC4/ARC4 (스트림 암호)

### 6.3 압축/필터
| 필터 | 설명 |
|------|------|
| Flate/Deflate | LZ77 기반 (설정 가능 윈도우) |
| LZW | Lempel-Ziv-Welch |
| CCITT Fax | Group 3 (1D), Group 4 (2D) |
| DCT/JPEG | JPEG 압축 |
| JBIG2 | 이진 이미지 압축 (globals 지원) |
| JPX/JPEG2000 | Wavelet 기반 |
| Brotli | 현대 압축 (레벨 0-11) |
| Run-Length | 런 렝스 인코딩 |
| ASCII85 | ASCII85 디코딩 |
| ASCIIHex | 16진수 디코딩 |
| Predictor | PNG/TIFF 예측자 |
| SGI Log | 그레이스케일 로그 인코딩 |
| Thunder | Thunder 압축 |

---

## 7. 특수 기능

### 7.1 바코드
**생성 지원 타입 (20+종)**:
Aztec, CODABAR, Code39, Code93, Code128, DataBar, DataBarExpanded, DataBarLimited, DataMatrix, DXFilmEdge, EAN8, EAN13, ITF, MaxiCode, PDF417, QRCode, MicroQR, RectMicroQR, UPC-A, UPC-E

**바코드 기능**: 생성, 디코딩 (페이지/pixmap/display list에서), 에러 정정 레벨, 여백, 사람이 읽을 수 있는 텍스트

### 7.2 OCR (Tesseract)
- Tesseract OCR 엔진 통합
- OCR 디바이스를 통한 텍스트 레이어 생성
- PDF-OCR 출력 (스캔 문서 → 검색 가능 PDF)

### 7.3 비디텍스트 (BiDi)
- Unicode Bidirectional Algorithm
- RTL/LTR 방향 감지
- 스크립트 인식 텍스트 처리

### 7.4 하이프네이션
- 언어별 하이프네이션 사전
- 단어 분리 패턴

### 7.5 페이지 트랜지션
Split, Blinds, Box, Wipe, Dissolve, Glitter, Fly, Push, Cover, Uncover, Fade

### 7.6 Story/Layout 엔진
- HTML/CSS 스타일 텍스트 레이아웃
- 멀티 페이지 스토리 배치
- 텍스트 오버플로우 처리
- 이미지 임베딩

### 7.7 Deskew (기울기 보정)
- 스캔 문서 기울기 감지
- 자동 보정 (회전)
- 테두리 처리 옵션

### 7.8 아카이브 처리
- ZIP, TAR, Directory 읽기
- 아카이브 열거 및 콘텐츠 접근
- GZIP 투명 압축 해제

### 7.9 Warp (왜곡 보정)
- 이미지/페이지 왜곡 보정

---

## 8. 커맨드라인 도구 (mutool)

| 도구 | 설명 |
|------|------|
| `mutool draw` | 문서를 이미지로 렌더링 |
| `mutool convert` | 문서 포맷 변환 |
| `mutool clean` | PDF 구문 정리/복구/최적화 |
| `mutool merge` | 여러 PDF 병합 |
| `mutool extract` | 임베디드 폰트/이미지 추출 |
| `mutool create` | 텍스트 페이지에서 PDF 생성 |
| `mutool show` | PDF 내부 객체 표시 |
| `mutool info` | PDF 리소스 정보 표시 |
| `mutool pages` | 페이지 정보 표시 |
| `mutool poster` | 큰 페이지를 타일로 분할 |
| `mutool sign` | 디지털 서명 관리 |
| `mutool trace` | 렌더링 디바이스 호출 추적 |
| `mutool run` | JavaScript 실행 환경 |
| `mutool grep` | 텍스트 검색 |
| `mutool audit` | PDF 사용 통계 |
| `mutool bake` | 폼을 정적 콘텐츠로 변환 |
| `mutool barcode` | 바코드 인코딩/디코딩 |
| `mutool recolor` | PDF 색공간 변환 |
| `mutool trim` | 페이지 콘텐츠 트리밍 |

---

## 9. 아키텍처 레이어

```
+--------------------------------------------------+
|              Application / Tools Layer            |
|  (mutool, viewers, language bindings)             |
+--------------------------------------------------+
|                   PDF Layer                       |
|  (pdf-annot, pdf-form, pdf-crypt, pdf-font,      |
|   pdf-page, pdf-xref, pdf-write, pdf-signature,  |
|   pdf-layer, pdf-js, pdf-zugferd, ...)            |
+--------------------------------------------------+
|            Fitz Core Layer (format-agnostic)      |
|  - Document abstraction (multi-format)            |
|  - Device model (draw, list, text, svg, ...)      |
|  - Rendering engine (path, glyph, image, shade)   |
|  - Color management (ICC, LCMS)                   |
|  - Font system (FreeType, HarfBuzz)               |
|  - Image codecs (JPEG, PNG, TIFF, JBIG2, JP2...) |
|  - Compression (Flate, LZW, Fax, Brotli, ...)    |
|  - Crypto (AES, RC4, MD5, SHA)                    |
|  - Text extraction (structured text)              |
|  - Geometry, Pixmap, Buffer, Stream, XML, JSON    |
+--------------------------------------------------+
|          Format Parsers                           |
|  XPS | HTML/EPUB | CBZ | SVG | MOBI | FB2 | TXT  |
+--------------------------------------------------+
|        Third-party Libraries                      |
|  FreeType | HarfBuzz | libjpeg | libpng | zlib   |
|  OpenJPEG | jbig2dec | LCMS2 | Brotli | Tesseract|
+--------------------------------------------------+
```

---

## 10. justpdf 구현 우선순위 제안

### Phase 1: Core (MVP)
- [ ] PDF 파서 (객체 모델, xref, 스트림)
- [ ] 기본 렌더링 (경로, 텍스트, 이미지)
- [ ] 기본 색공간 (Gray, RGB, CMYK)
- [ ] 기본 폰트 (Type1, TrueType)
- [ ] 기본 압축 (Flate, DCT)
- [ ] 텍스트 추출
- [ ] PDF 생성/저장

### Phase 2: 완성도
- [ ] 어노테이션 (전체)
- [ ] 폼 필드
- [ ] 암호화/복호화 (RC4, AES)
- [ ] 북마크/아웃라인
- [ ] 이미지 포맷 확장 (PNG, TIFF, JBIG2, JP2)
- [ ] ICC 색상 관리
- [ ] 폰트 서브세팅

### Phase 3: 고급 기능
- [ ] 디지털 서명
- [ ] JavaScript 지원
- [ ] Optional Content (레이어)
- [ ] Redaction
- [ ] Linearized PDF
- [ ] 증분 저장

### Phase 4: 확장
- [ ] XPS, EPUB, HTML 지원
- [ ] SVG 입출력
- [ ] Office 포맷
- [ ] OCR 통합
- [ ] 바코드
