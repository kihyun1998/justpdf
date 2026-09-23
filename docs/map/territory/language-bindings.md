# Aggregate — 언어 바인딩

**This note owns no detail.** 다른 언어에서 Rust API를 부르기 위한 얇은 어댑터 네 개의 관계.

| Concept | Note |
|---|---|
| C ABI + 손으로 쓴 헤더 | [ffi-binding](ffi-binding.md) |
| PyO3 / maturin | [python-binding](python-binding.md) |
| wasm-bindgen 범용 모듈 | [wasm-binding](wasm-binding.md) |
| napi-rs | [node-binding](node-binding.md) |

## Why they sit together
넷 다 **같은 좁은 core+render 부분집합**(열기·인증·페이지 수·텍스트 추출·PNG 렌더·몇몇 Info 문자열)을 각자 다시 감싼다. 서로 호출하지 않고 [파사드](facade.md)도 쓰지 않으므로, core·render 공개 API가 바뀌면 네 곳을 따로 고쳐야 한다. [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md)가 이 네 개를 "워크스페이스 밖 일반 바인딩"으로 묶는다.

## ADR-0002와 저장소가 어긋나는 지점
ADR-0002는 네 바인딩 모두가 워크스페이스 멤버에서 제외되고 각자 빈 `[workspace]` 블록을 가진다고 적는다. 2026-09-23 저장소 상태:
- `justpdf-ffi`는 루트 `Cargo.toml`의 `members`에 **들어 있다**(ADR보다 먼저 추가됨).
- `justpdf-ffi`·`justpdf-wasm`에는 `[workspace]` 블록이 없다.
- 그 결과 `cargo metadata --manifest-path justpdf-wasm/Cargo.toml`이 "current package believes it's in a workspace when it's not"로 실패한다 — wasm 바인딩은 현재 단독 빌드가 되지 않는다.
- python·node만 ADR대로(빈 `[workspace]`, 자체 `Cargo.lock`) 분리되어 있다.

확인 명령: `grep -n members Cargo.toml; grep -c '^\[workspace\]' justpdf-{ffi,python,wasm,node}/Cargo.toml`.

어느 쪽(ADR 또는 매니페스트)을 고칠지는 결정이 필요하다. 이 맵은 ADR을 고치지 않는다.

## CI가 보지 않는다
CI는 `--workspace` 명령만 돌므로 ffi는 호스트 플랫폼에서만 컴파일되고, python·wasm·node는 **어떤 CI 작업도 빌드하지 않는다**(maturin·napi·wasm32 작업 없음). 조용히 깨지는 부류다 — [CI](ci.md).

Tracked: #36 (justpdf-wasm 매니페스트·ADR-0002)
