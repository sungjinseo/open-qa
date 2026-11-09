FROM rust:1-alpine@sha256:44819b27be4c97b0e4d7203bd2e02c4820f97700211b6a37fb4c246a2de59383
RUN apk add --no-cache musl-dev gcc g++ make git
ARG OPENGREP_VERSION=v1.33.1
# 릴리스 자산의 sha256을 고정한다(런타임 Dockerfile은 cosign으로 검증한다). 버전을 올리면 값도 갱신한다.
ADD --checksum=sha256:aa03c3bfff105dc122cbbe6e239dc39a5cae486f1837bba3af7a16bca173f8d6 --chmod=755 https://github.com/opengrep/opengrep/releases/download/${OPENGREP_VERSION}/opengrep_musllinux_x86 /usr/local/bin/opengrep
RUN git config --global user.email test@example.com && git config --global user.name test
