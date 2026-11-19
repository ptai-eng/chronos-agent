FROM ubuntu:26.04

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl git tar \
    && rm -rf /var/lib/apt/lists/*

ARG CHRONOS_VERSION=v1.8.0
RUN curl -fsSL "https://github.com/ptai-eng/chronos-agent/releases/download/${CHRONOS_VERSION}/chronos-linux-x64.tar.gz" \
    | tar -xz -C /usr/local/bin \
    && chmod +x /usr/local/bin/chronos /usr/local/bin/chronos-mcp

WORKDIR /workspace
ENTRYPOINT ["chronos-mcp"]
CMD ["--workspace", "/workspace", "--no-watch"]
