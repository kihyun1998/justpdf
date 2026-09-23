# 바코드·QR 생성

## What it is
QR(`qrcode` 크레이트), Code128·EAN-13·Code39(자체 구현), DataMatrix·PDF417·Aztec(자체 구현)을 이미지·PNG로 생성한다. PDF를 만들지 않는다.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — render 없이 빌드된다.

## Design model
- PDF417 코드워드 패턴은 "a simplified encoding"이다.
- `barcode/mod.rs`에 Reed-Solomon 코드가 없다 — DataMatrix·PDF417·Aztec 출력은 규격 오류 정정이 없어 스캔되지 않을 수 있다(추론).

## Code
- `justpdf-special/src/barcode/mod.rs` — `BarcodeType`, `BarcodeImage`, `generate_qr`, `generate_qr_png`, `generate_barcode`, `generate_barcode_png`, `generate_datamatrix`, `generate_pdf417`, `generate_aztec`

## Reference behaviour
**None.** 실제 스캐너로 확인한 기록 없음.

## Cross-cutting invariants
**None.**

## Blast radius
- [크레이트 레이어링](crate-layering.md) — `barcode` 단독 빌드는 격리 스크립트가 확인한다.

## Known holes / open
- 2D 코드의 오류 정정 부재(위).
- Tracked: #55 (2D 바코드 오류 정정)
