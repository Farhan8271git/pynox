# Pynox runtime image. The build context holds one extracted bundle per
# architecture, bundle-amd64/ and bundle-arm64/; scripts/image_context.py
# prepares it. There is no RUN step, so cross-architecture builds need no
# emulation and the image contains exactly the bundle.
ARG BASE=debian:bookworm-slim@sha256:7c7b2c966bc9ee8cedfeef67e0e279108992c77681fa595db4a9d65c06ccc587
FROM ${BASE}
ARG TARGETARCH
ARG VERSION=0.1.0
ARG PYTHON=3.11
LABEL org.opencontainers.image.title="pynox" \
      org.opencontainers.image.description="A Python with the sandbox built in, made for the code your agents write" \
      org.opencontainers.image.source="https://github.com/ferrumio/pynox" \
      org.opencontainers.image.licenses="Apache-2.0" \
      org.opencontainers.image.version="${VERSION}-py${PYTHON}"
COPY bundle-${TARGETARCH}/ /opt/pynox/
ENV PATH=/opt/pynox/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
# Landlock and seccomp need no privilege; run as an unprivileged user.
USER 65532:65532
WORKDIR /app
ENTRYPOINT ["pynox"]
CMD ["--help"]
