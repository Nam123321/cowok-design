#!/usr/bin/env bash
# Story 12.3: Hardened Non-Root Container Entrypoint with /dev/shm Guard,
# Read-Only Bind-Mount SQLite WAL Staging & SIGTERM/SIGINT PGID Child Reaper.
set -euo pipefail

# 1. Enforce non-root execution (uid 10001) unless explicitly opted in for diagnostics
if [[ "$(id -u)" -eq 0 && "${COWOK_ALLOW_ROOT:-0}" != "1" ]]; then
  echo "error [ROOT_EXECUTION_FORBIDDEN]: container must run as non-root uid 10001" >&2
  exit 1
fi

# 2. Ensure writable runtime staging directories under ${RUNTIME_ROOT}
RUNTIME_ROOT="${COWOK_RUNTIME_ROOT:-/tmp/cowok-runtime}"
export XDG_DATA_HOME="${XDG_DATA_HOME:-${RUNTIME_ROOT}/data}"
export XDG_STATE_HOME="${XDG_STATE_HOME:-${RUNTIME_ROOT}/state}"
export XDG_CACHE_HOME="${XDG_CACHE_HOME:-${RUNTIME_ROOT}/cache}"
mkdir -p "${XDG_DATA_HOME}" "${XDG_STATE_HOME}" "${XDG_CACHE_HOME}" "${RUNTIME_ROOT}/chrome"
chmod 0700 "${RUNTIME_ROOT}" "${XDG_DATA_HOME}" "${XDG_STATE_HOME}" "${XDG_CACHE_HOME}" "${RUNTIME_ROOT}/chrome" 2>/dev/null || true

# 3. Probe workspace writability atomically via mktemp (`O_CREAT | O_EXCL`)
#    (handles :ro and mismatched-UID Linux bind mounts cleanly without clobbering symlinks)
WORKSPACE_DIR="${UXUI_OS_WORKSPACE_ROOT:-/workspace}"
if [[ -d "${WORKSPACE_DIR}" ]]; then
  if PROBE_FILE="$(mktemp "${WORKSPACE_DIR}/.cowok-uid-probe.XXXXXX" 2>/dev/null)"; then
    rm -f "${PROBE_FILE}" 2>/dev/null || true
  else
    export COWOK_WAL_READ_ONLY_FALLBACK=1
    export XDG_DATA_HOME="${RUNTIME_ROOT}/data"
    export XDG_STATE_HOME="${RUNTIME_ROOT}/state"
    export XDG_CACHE_HOME="${RUNTIME_ROOT}/cache"
    mkdir -p "${XDG_DATA_HOME}" "${XDG_STATE_HOME}" "${XDG_CACHE_HOME}"
    chmod 0700 "${XDG_DATA_HOME}" "${XDG_STATE_HOME}" "${XDG_CACHE_HOME}" 2>/dev/null || true
  fi
fi

# 4. Probe /dev/shm capacity (threshold: 256 MiB = 268435456 bytes = 262144 KiB)
SHM_THRESHOLD_KIB=262144
SHM_AVAIL_KIB=0
if [[ -d "/dev/shm" ]]; then
  SHM_AVAIL_KIB="$(df -Pk /dev/shm 2>/dev/null | awk 'NR==2 {print $4}' || echo 0)"
fi
if [[ ! "${SHM_AVAIL_KIB}" =~ ^[0-9]+$ ]] || (( SHM_AVAIL_KIB < SHM_THRESHOLD_KIB )); then
  export COWOK_DISABLE_DEV_SHM=1
  export COWOK_CHROMIUM_DISABLE_DEV_SHM=1
  if [[ "${COWOK_CHROMIUM_EXTRA_FLAGS:-}" != *"--disable-dev-shm-usage"* ]]; then
    export COWOK_CHROMIUM_EXTRA_FLAGS="${COWOK_CHROMIUM_EXTRA_FLAGS:+${COWOK_CHROMIUM_EXTRA_FLAGS} }--disable-dev-shm-usage"
  fi
fi

# 5. Resolve canonical binary and normalize optional alias invocation
BIN_PATH="/usr/local/bin/cowok-design"
if [[ $# -gt 0 ]]; then
  case "$1" in
    cowok-design|uiux-os|harness-daemon)
      shift
      ;;
  esac
fi

SELF_PGID="$(ps -o pgid= -p $$ 2>/dev/null | tr -d ' ' || true)"
CHILD_PID=""
CHILD_PGID=""
SIGNAL_EXIT_CODE=0

# Returns 0 iff PID exists AND is not already a zombie ('Z') or dead ('X').
# Prevents `kill -0` from spinning 5 seconds on a zombie child waiting to be reaped by `wait`.
is_pid_running_non_zombie() {
  local pid="$1"
  if [[ -r "/proc/${pid}/stat" ]]; then
    local stat_line stat_rest state
    stat_line="$(cat "/proc/${pid}/stat" 2>/dev/null || true)"
    if [[ -z "${stat_line}" ]]; then
      return 1
    fi
    stat_rest="${stat_line##*) }"
    state="${stat_rest%% *}"
    [[ "${state}" != "Z" && "${state}" != "X" ]]
    return $?
  fi
  local ps_stat
  ps_stat="$(ps -o stat= -p "${pid}" 2>/dev/null | tr -d ' ' || true)"
  if [[ -z "${ps_stat}" ]]; then
    return 1
  fi
  [[ "${ps_stat:0:1}" != "Z" && "${ps_stat:0:1}" != "X" ]]
}

reap_children() {
  trap - TERM INT
  local sig="$1"
  if [[ "${sig}" == "INT" ]]; then
    SIGNAL_EXIT_CODE=130
  else
    SIGNAL_EXIT_CODE=143
  fi

  if [[ -n "${CHILD_PGID}" && "${CHILD_PGID}" != "${SELF_PGID}" ]]; then
    kill -TERM "-${CHILD_PGID}" 2>/dev/null || true
  elif [[ -n "${CHILD_PID}" ]]; then
    kill -TERM "${CHILD_PID}" 2>/dev/null || true
  fi

  if [[ -n "${CHILD_PID}" ]]; then
    local waited=0
    while is_pid_running_non_zombie "${CHILD_PID}" && (( waited < 50 )); do
      sleep 0.1
      waited=$((waited + 1))
    done
    if is_pid_running_non_zombie "${CHILD_PID}"; then
      if [[ -n "${CHILD_PGID}" && "${CHILD_PGID}" != "${SELF_PGID}" ]]; then
        kill -KILL "-${CHILD_PGID}" 2>/dev/null || true
      else
        kill -KILL "${CHILD_PID}" 2>/dev/null || true
      fi
    fi
    wait "${CHILD_PID}" 2>/dev/null || true
  fi
  exit "${SIGNAL_EXIT_CODE}"
}

trap 'reap_children TERM' TERM
trap 'reap_children INT' INT

if command -v setsid >/dev/null 2>&1; then
  setsid "${BIN_PATH}" "$@" &
  CHILD_PID=$!
  CHILD_PGID="${CHILD_PID}"
else
  "${BIN_PATH}" "$@" &
  CHILD_PID=$!
  CHILD_PGID="$(ps -o pgid= -p "${CHILD_PID}" 2>/dev/null | tr -d ' ' || true)"
fi

set +e
wait "${CHILD_PID}"
EXIT_CODE=$?
set -e
exit "${EXIT_CODE}"
