# Vex - AI Agent Target PaaS

## Build & Test

```bash
cargo build                          # 전체 빌드
cargo test                           # 전체 테스트
cargo test -p vex-builder            # 특정 크레이트 테스트
cargo clippy -- -D warnings         # 린트
cargo fmt --check                    # 포맷 체크
```

## Architecture

4-crate workspace:
- `vex-core`: 공유 타입, DB 모델, 에러, 요청/응답 스키마
- `vex-builder`: 프로젝트 감지 + Dockerfile 생성
- `vex-server`: axum API 서버 + 리버스 프록시
- `vex-cli`: clap CLI 바이너리

## Code Conventions

- 주석 금지. 코드로 의도 표현
- `as` 타입 단언 금지
- 에러: `thiserror`(라이브러리), `anyhow`(바이너리)
- 에러 메시지: 소문자 시작, 마침표 없음
- `mod.rs`는 re-export만
- `pub`은 최소한으로

## Key Dependencies

- axum 0.8, bollard, sqlx (postgres), clap (derive)
- tokio, reqwest, tracing, dashmap, uuid v7
