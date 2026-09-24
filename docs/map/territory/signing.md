# 디지털 서명 (CMS·ByteRange)

## What it is
원본 바이트를 그대로 둔 채 서명 사전·위젯이 들어간 증분 구간을 덧붙이고, `/Contents` 자리표시자를 남겨 ByteRange를 계산한 뒤, 그 범위의 다이제스트로 CMS SignedData(`adbe.pkcs7.detached`)를 만들어 자리표시자에 hex로 채운다. DER은 손으로 만든다.

## Governing decisions
**None.**

## Design model
- 서명 키는 RSA PKCS#8만 받는다. `/Contents` 자리표시자는 `PLACEHOLDER_SIZE` 바이트.
- `contents_offset`을 `<` 바로 뒤로 잡아 `<`·`>` 구분자가 서명 범위에 들어간다(추론: 구분자까지 제외하는 관행과 다름).
- `fix_byte_range`는 파일 전체에서 자리표시자 텍스트의 첫 일치를 패치한다.
- **증분 구간이 문서에 연결되지 않는다**: 필드를 `/AcroForm /Fields`에 넣지 않고, Catalog를 갱신하지 않고, 위젯을 페이지 `/Annots`에 넣지 않는다. 모듈 주석의 "3. Updated AcroForm 4. Updated Catalog"는 구현되지 않았다. `/M`을 쓰지 않고 `contact_info`는 버려진다.
- 새 trailer는 `incremental_trailer`로 만든다 — [증분 trailer](../invariant/incremental-trailer.md). 암호화된 입력은 `UnsupportedEncryption`으로 거부한다(비밀번호를 받지 않아 덧붙일 객체를 암호화할 키가 없다 — #26 메인테이너 판단).
- 서명자 이름 등은 자체 `escape_pdf_string`(`( ) \`만)으로 Rust `&str`의 UTF-8을 literal에 쓴다. `write_pdf_value`는 이름·문자열을 **전혀 이스케이프하지 않는다** — [객체 구문 왕복](../invariant/object-syntax-roundtrip.md), [텍스트 문자열 인코딩](../invariant/text-string-encoding.md).

## Code
- `justpdf-core/src/sign/sign_pdf.rs` — `sign_pdf`, `build_pdf_with_placeholder`, `create_cms_signed_data`, `build_signer_info`, `build_utctime_now`, `write_pdf_value`, `escape_pdf_string`, `fix_byte_range`, `PLACEHOLDER_SIZE`
- `justpdf-core/src/sign/byterange.rs` — `compute_byterange_digest`, `detect_modification_after_signing`

## Reference behaviour
**None.** 비교 대상 조항: ISO 32000-2 §12.8.1(ByteRange·Contents), §12.8.3. 제3자 검증기(Acrobat 등)로 확인한 기록 없음.

## Cross-cutting invariants
- [객체 구문 왕복](../invariant/object-syntax-roundtrip.md) — `write_pdf_value`, `escape_pdf_string`.
- [텍스트 문자열 인코딩](../invariant/text-string-encoding.md) — `/Name`·`/Reason`·`/Location`.
- [증분 trailer](../invariant/incremental-trailer.md) — 쓰기 쪽 사이트.

## Blast radius
- [증분 저장](incremental-save.md) — 같은 방식의 다른 구현. 한쪽에서 발견한 결함을 다른 쪽에서 찾는다.
- [xref](xref.md) — 덧붙인 구간을 읽는 쪽.
- [서명 외관](signature-appearance.md) — `build_pdf_with_placeholder`가 부른다.
- [타임스탬프](timestamps.md) — `build_signer_info`가 토큰을 서명되지 않은 속성으로 넣는다.
- [서명 감지](signature-detection.md), [서명 검증](signature-verification.md) — 결과를 읽는 쪽.
- [CLI](cli.md) — `sign` 서브커맨드는 이 모듈을 부르지 않는다(스텁).

## Known holes / open
- 서명 → 검증 왕복 테스트가 없다.
- CLI `sign`은 "not yet fully implemented"를 출력하고 성공 코드로 끝난다. CLI는 `--cert`를 받지만 core에 PKCS#12 파서가 없다.
- 암호화 문서는 서명할 수 없다(위).
- Tracked: #29 (손으로 쓰는 구문 이스케이프), #33 (텍스트 문자열 인코딩), #37 (서명 연결·CLI sign)
