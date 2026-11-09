FROM rust:1-alpine
RUN apk add --no-cache musl-dev gcc g++ make git
ARG OPENGREP_VERSION=v1.33.1
ADD https://github.com/opengrep/opengrep/releases/download/${OPENGREP_VERSION}/opengrep_musllinux_x86 /usr/local/bin/opengrep
RUN chmod +x /usr/local/bin/opengrep
RUN git config --global user.email test@example.com && git config --global user.name test
