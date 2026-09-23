# 텍스트 출력 형식

## What it is
추출된 `PageText`를 plain / HTML / JSON / Markdown으로 서식화한다. CLI `text --format`이 유일한 소비처다.

## Governing decisions
**None.**

## Design model
- JSON은 손으로 만든 이스케이프(`json_string`)를 쓴다.
- Markdown은 헤딩 감지 없이 블록 텍스트이며, 여러 페이지면 `## Page N`을 붙인다.

## Code
- `justpdf-core/src/text/format.rs` — `OutputFormat`, `format_page`, `format_pages`, `format_html`, `format_json`, `format_markdown`, `json_string`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [텍스트 추출](text-extraction.md) — 입력 구조.
- [CLI](cli.md) — `cmd_text`의 형식 이름 매핑. 형식을 추가하면 CLI 도움말도.
- [게시 문서](published-docs.md) — CLI 문서의 형식 목록.

## Known holes / open
**None.** 이 노트를 쓰며 발견된 구멍은 없다.
