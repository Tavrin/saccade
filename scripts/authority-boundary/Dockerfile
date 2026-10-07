# Build the binary separately; this runtime recipe never builds on the root disk.
FROM python:3.12-slim-trixie@sha256:05cda9777409a9c3ffddd94a4c476b79f0769a0b4857f0c7ed9226b6800b0d6f
COPY saccade /usr/local/bin/saccade
COPY harness.py /opt/authority/harness.py
COPY verifier /opt/authority/verifier
COPY verifier-entrypoint /usr/bin/ssh-keygen
USER 10001:10001
WORKDIR /tmp
ENTRYPOINT ["python3", "/opt/authority/harness.py", "--bin", "/usr/local/bin/saccade"]
