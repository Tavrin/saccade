#!/usr/bin/env bash
# Clean installed-binary acceptance. Cargo and repository examples are absent.
set -euo pipefail
if [[ $# != 2 ]]; then
  echo 'usage: run-playbooks-container.sh BINARY NEW_EVIDENCE_DIR' >&2
  exit 2
fi
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
binary=$(realpath "$1")
out=$(realpath -m "$2")
[[ ! -e "$out" ]] || { echo 'Evidence directory must be new.' >&2; exit 2; }
mkdir -p "$out/kit/scripts" "$out/kit/integrations/playwright"
cp "$root/scripts/"{run-playbooks.py,gen-playbook-fixtures.py,gen-guide-fixtures.py} "$out/kit/scripts/"
cp -R "$root/playbooks" "$out/kit/"
cp "$root/integrations/playwright/"{sweep.cjs,stabilize.cjs,matcher.cjs} "$out/kit/integrations/playwright/"
docker build -t saccade-playbooks:local -f "$root/scripts/playbooks/Dockerfile" "$root/scripts/playbooks"
container_user="$(id -u):$(id -g)"
# A rootless daemon maps container root to the invoking host user.
if docker info --format '{{json .SecurityOptions}}' | grep -q 'name=rootless'; then
  container_user=0:0
fi
docker run --rm --network none --read-only --tmpfs /tmp:rw,size=256m \
  --shm-size=256m --user "$container_user" \
  -v "$binary:/usr/local/bin/saccade:ro" -v "$out/kit:/kit:ro" -v "$out:/evidence" \
  saccade-playbooks:local --bin /usr/local/bin/saccade --out /evidence/run --require-all
