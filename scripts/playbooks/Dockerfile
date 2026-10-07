FROM mcr.microsoft.com/playwright:v1.51.1-noble@sha256:146d046a8d79a1b3a87596c4457b0b1c47f811bf4fc2cc1b99e873ae7f1cbbbd
USER root
RUN apt-get update && apt-get install --yes --no-install-recommends python3 ffmpeg \
    && rm -rf /var/lib/apt/lists/*
RUN npm install --prefix /opt/playbook-node --no-audit --no-fund @playwright/test@1.51.1
ENV NODE_PATH=/opt/playbook-node/node_modules
WORKDIR /kit
ENTRYPOINT ["python3", "/kit/scripts/run-playbooks.py"]
