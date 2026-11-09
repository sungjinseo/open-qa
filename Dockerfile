FROM rust:1-alpine@sha256:44819b27be4c97b0e4d7203bd2e02c4820f97700211b6a37fb4c246a2de59383 AS build
RUN apk add --no-cache musl-dev gcc g++ make
WORKDIR /w
COPY Cargo.toml Cargo.lock ./
COPY src src
RUN cargo build --release --locked

FROM alpine:3@sha256:f59abe656b692d901e184be3445e084786602108599d2a51d229c0f5a48c71bf AS fetch
RUN apk add --no-cache cosign curl
ARG OPENGREP_VERSION=v1.33.1
WORKDIR /f
RUN base=https://github.com/opengrep/opengrep/releases/download/${OPENGREP_VERSION}/opengrep_musllinux_x86 \
 && curl -fsSLO "$base" && curl -fsSLO "$base.cert" && curl -fsSLO "$base.sig" \
 && cosign verify-blob --certificate opengrep_musllinux_x86.cert --signature opengrep_musllinux_x86.sig \
      --certificate-identity-regexp 'https://github.com/opengrep/opengrep/.*' \
      --certificate-oidc-issuer https://token.actions.githubusercontent.com opengrep_musllinux_x86

FROM alpine:3@sha256:f59abe656b692d901e184be3445e084786602108599d2a51d229c0f5a48c71bf
RUN apk add --no-cache git ca-certificates && adduser -D -u 10001 qualgate
COPY --from=fetch --chmod=755 /f/opengrep_musllinux_x86 /usr/local/bin/opengrep
COPY --from=build /w/target/release/qualgate /usr/local/bin/qualgate
COPY rules /rules
ENV HOME=/tmp
USER qualgate
ENTRYPOINT ["/usr/local/bin/qualgate"]
