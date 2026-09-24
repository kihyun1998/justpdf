# 객체 복호화 (읽기)

## What it is
인증된 문서에서 객체를 로드할 때 문자열과 스트림 데이터를 객체별 키로 복호화한다. 스트림별 `/Crypt` 필터와 `DecodeParms /Name`을 처리한다.

## Governing decisions
**None.**

## Design model
- 훅 위치는 [문서 접근](document-access.md)의 `load_object`(일반 객체)와 `load_compressed_object`(ObjStm 전체)다.
- **ObjStm 안 객체는 두 번 복호화된다**(추론): 스트림 전체를 복호화하고, 꺼낸 객체에 대해 자기 번호로 `decrypt_object`가 다시 돈다.
- 스트림 사전 안의 문자열은 문자열 방식으로 복호화한다 — 스트림 데이터의 방식(`/Crypt` 필터, Identity 포함)과 따로다(MuPDF `pdf_crypt_obj`도 사전 문자열에 `strf`를 쓴다). 복호화되지 않는 문자열(AES 길이·패딩 오류)은 쓰인 그대로 둔다(`decrypt_strings_or_keep`) — 사전 문자열 하나 때문에 콘텐츠·폰트·이미지 스트림 전체가 열리지 않게 하지 않으려는 것으로, MuPDF도 경고만 하고 원래 문자열을 둔다. ISO 32000-1 §7.6.1(원문 대조)의 예외는 trailer `/ID`, `/Encrypt` 사전의 문자열, 스트림 **안**의 문자열뿐이다.
- `/Type /XRef` 스트림은 데이터·사전 모두 복호화하지 않는다(`is_xref_stream`) — §7.5.8.2 "The cross-reference stream shall not be encrypted and strings appearing in the cross-reference stream dictionary shall not be encrypted". 그 사전에는 `/ID`가 들어 있다.
- 이 규칙 이전의 justpdf는 스트림 사전 문자열을 평문으로 썼다. 그렇게 쓴 암호화 파일을 지금 읽으면 그 문자열(예: 첨부파일 `/Params`의 `/ModDate`·`/CheckSum`)은 깨져 읽힌다 — qpdf도 같게 읽는다.
- 서명 `/Contents`는 스펙상 암호화되지 않지만 `resolve`를 지나며 복호화된다(추론) — [서명 감지](signature-detection.md).

## Code
- `justpdf-core/src/crypto/decrypt.rs` — `decrypt_object`, `decrypt_strings_or_keep`, `is_xref_stream`, `decrypt_bytes`, `stream_crypt_method`, `remove_crypt_filter`
- `justpdf-core/tests/integration.rs` — `test_stream_dictionary_strings_decrypt_in_third_party_files`
- `justpdf-core/src/parser.rs` — `load_object`, `load_compressed_object`

## Reference behaviour
ISO 32000-1:2008 원문(Adobe 무료 사본, 2026-09-24)과 대조: §7.6.1의 암호화 예외 목록(trailer `/ID`, `/Encrypt` 사전의 문자열, 스트림 안의 문자열)과 §7.5.8.2의 xref 스트림 규칙 — 스트림 사전 문자열과 xref 스트림 처리가 이를 따른다. qpdf 12.3.2가 만든 R3·R4·R6 파일(Known holes의 픽스처)로 스트림 사전 문자열 복호화를 확인한다. 아직 대조하지 않은 조항: ISO 32000-2 §7.5.7(object stream 안 문자열은 따로 암호화하지 않음), §7.6.2.

## Cross-cutting invariants
**None.**

## Blast radius
- [object streams](object-streams.md) — 이중 복호화의 다른 쪽.
- [객체 암호화](object-encryption.md) — 쓰기 쪽 짝. 어느 값을 암호화하는지의 규칙을 공유해야 한다.
- [암호화 모델](encryption-model.md) — 방식 선택.
- [서명 감지](signature-detection.md), [서명 검증](signature-verification.md) — `/Contents`.

## Known holes / open
- 암호화 + object stream 파일 테스트가 없다.
- **스트림 사전 문자열 픽스처**(`justpdf-core/tests/fixtures/`): qpdf 12.3.2(pikepdf 10.13.0)로 만든 R3(RC4-128)·R4(AES-128)·R6(AES-256) 파일. 사용자 `user`, 소유자 `owner`. catalog의 `/Probe`가 사전에 `/Note (hello-note)`를 둔 스트림(데이터 `stream body`)이고, 파일 바이트에 `hello-note` 평문이 없다. 우리 코드와 독립된 판정자다.
  - `stream_dict_string_r3.pdf` (sha256 `18fd03e9d81951eb9e0e8e91790c1b3cb81537ab489c833949e88e462b413e4a`), `stream_dict_string_r4.pdf` (`8b6205dac67f48caabc4dc339a9305747f96f1e9769e0259a0fe36696b4abacc`), `stream_dict_string_r6.pdf` (`0d6b1cd4b230bb1276da294b5ebf51374ba7f5ffffa43dd7befd35449fce9e74`)
  - 다시 만들기: `pdf = pikepdf.new(); pdf.add_blank_page(); st = pikepdf.Stream(pdf, b"stream body"); st.Note = pikepdf.String("hello-note"); pdf.Root.Probe = pdf.make_indirect(st); pdf.save(path, encryption=pikepdf.Encryption(user="user", owner="owner", R=R, aes=R>=4, metadata=R>=4), object_stream_mode=pikepdf.ObjectStreamMode.disable)`. 키·IV가 난수라 바이트는 매번 달라진다.
- `/StrF`와 `/StmF`가 다른 파일(예: Identity 스트림 필터)의 사전 문자열을 확인한 테스트가 없다 — 사전 문자열을 스트림 방식으로 복호화하도록 바꿔도 현재 테스트는 모두 통과한다.
- Tracked: #32 (ObjStm 이중 복호화)
