# 언어 바인딩 크레이트는 워크스페이스 외부

`justpdf-ffi`, `justpdf-python`, `justpdf-wasm`, `justpdf-node`는 루트 `Cargo.toml`의 워크스페이스 멤버에서 **의도적으로 제외**되며, 각자의 `Cargo.toml`에 `[workspace]` 빈 블록을 명시해 자기 자신을 별도 워크스페이스로 분리한다. 반면 `justpdf-compress-wasm`은 워크스페이스 안에 있다.

## 기준: 1급 product vs 일반 언어 바인딩

- **워크스페이스 안 (1급 product)**: `justpdf-compress-wasm`. "브라우저에서 돌아가는 PDF 압축 도구"라는 완성된 제품. `core`와 함께 CI/테스트/버전을 끌고 간다.
- **워크스페이스 밖 (일반 바인딩)**: `ffi/python/wasm/node`. 다른 언어 사용자를 위한 일반 바인딩으로, 각자 다른 레지스트리(crates.io/PyPI/npm)와 빌드 파이프라인을 가진다. 워크스페이스에 묶으면 lockfile과 feature resolver가 충돌하므로 분리.

## 새 크레이트를 추가할 때

같은 wasm/cdylib이라도 정체성을 따져 결정한다.

- "이건 그 자체로 사용자가 소비하는 완성된 도구인가?" → 워크스페이스 안.
- "이건 Rust API를 다른 언어에서 호출하기 위한 얇은 어댑터인가?" → 워크스페이스 밖.

## 트레이드오프

전부 워크스페이스에 묶는 대안은 단일 lockfile/공유 deps로 단순하지만, 바인딩별 빌드 도구(maturin, napi-build, wasm-pack)가 워크스페이스 빌드 시 서로 간섭한다. 분리 시 lockfile이 늘어나는 비용보다 빌드 격리 이득이 크다고 판단했다.
