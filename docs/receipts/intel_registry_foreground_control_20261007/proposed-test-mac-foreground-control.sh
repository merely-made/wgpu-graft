#!/usr/bin/env bash
# Run every demo-mac integration-test mode in sequence, propagate
# their per-mode exit codes (each test mode now exits 1 on FAIL),
# and surface an aggregate PASS/FAIL summary plus a nonzero exit
# code if any mode fails. Designed for CI and for "did the refactor
# break anything?" smoke runs.
#
# Each mode is wrapped in a wall-clock alarm via perl(1) â€” internal
# step deadlines (typically 5s per step) should keep the process
# under the alarm even on a slow runner, but the alarm guarantees
# no test mode can hang the whole suite. The alarm executes the built
# demo directly, so it owns the process that must be terminated.
#
# Each \`--*-test\` mode runs headless by default (no visible window,
# `NSApplicationActivationPolicyProhibited` so the developer's
# frontmost app keeps focus and no Dock icon flashes), so this
# script is safe to run while you're working on something else.
# To watch a failing test in real time, run the failing mode
# manually with \`--visible\`:
#     cargo run -p demo-mac -- --interaction-state-test --visible
#
# Requires:
#   - cargo on PATH
#   - a logged-in macOS user session (the AppKit run loop still
#     needs a WindowServer connection even for hidden windows)
#
# ScreenCaptureKit is intentionally a separate hardware suite because macOS
# must grant Screen Recording permission to the generated app ahead of time.
# By default the app is ad-hoc signed with an explicit stable designated
# requirement, so the grant survives rebuilds without giving a background
# runner access to a private signing key. Set SCRY_MAC_CODESIGN_IDENTITY to use
# a provisioned Developer ID/Apple Development identity instead:
#     CAPTURE=1 bash scripts/test-mac.sh
# Self-hosted hardware runners should also make the AppKit tests visible. This
# makes the script wrap the built binary in a stable .app and use
# LaunchServices, because a binary spawned directly from the runner's idle
# process context does not reliably advance WKWebView or ScreenCaptureKit:
#     VISIBLE=1 CAPTURE=1 bash scripts/test-mac.sh

set -uo pipefail

cd "$(dirname "$0")/.."

# ScreenCaptureKit can keep delivering status-only samples when a logged-in
# runner's display has idled, even though AppKit and WKWebView still advance.
# Mark the session active and hold display/system idle sleep for the lifetime
# of the capture suite. Keep ordinary, non-capture developer runs unchanged.
if [[ "${CAPTURE:-0}" = "1" ]] && command -v caffeinate >/dev/null 2>&1; then
    caffeinate -u -t 5 &
    # Match the registry-only harness that produced the release receipt: give
    # WindowServer time to wake before starting the display/system hold.
    sleep 2
    caffeinate -dims -w $$ &
    echo "==> ScreenCaptureKit display-awake hold active"
fi

# Per-mode wall-clock cap. The internal step deadlines fail any
# stuck step in 5s, so 60s is plenty for a healthy run.
TIMEOUT="${TIMEOUT:-60}"

MODES=(
    --scripted
    --browser-test
    --interaction-state-test
    --pointer-input-test
    --incognito-test
    --download-test
    --profile-test
    --two-tabs
)

if [[ "${CAPTURE:-0}" = "1" ]]; then
    MODES+=(--capture-test --capture-test+resize)
fi

# Optional diagnosis is a separate bounded two-mode run. Preserve the default
# battery and its original fixture; activity probes can affect scheduling.
if [[ "${CAPTURE_ACTIVITY:-0}" = "1" ]]; then
    MODES=(--capture-test --capture-test+resize)
fi

# Build once so each `cargo run` invocation skips compile overhead.
echo "==> building demo-mac"
if ! cargo build --locked -q; then
    # A failed rebuild must not run a leftover binary as source qualification.
    exit 1
fi

DEMO_BIN="${CARGO_TARGET_DIR:-target}/debug/demo-mac"
if [[ ! -x "$DEMO_BIN" ]]; then
    echo "demo-mac executable not found at $DEMO_BIN" >&2
    exit 1
fi

DEMO_APP=""
if [[ "$(uname -s)" = "Darwin" && "${VISIBLE:-0}" = "1" ]]; then
    DEMO_APP="${SCRY_MAC_APP:-${CARGO_TARGET_DIR:-target}/debug/scry-demo-mac-hardware.app}"
    case "$DEMO_APP" in
        *.app) ;;
        *)
            echo "SCRY_MAC_APP must end in .app: $DEMO_APP" >&2
            exit 1
            ;;
    esac
    mkdir -p "$DEMO_APP/Contents/MacOS"
    cp "$DEMO_BIN" "$DEMO_APP/Contents/MacOS/demo-mac.new"
    chmod +x "$DEMO_APP/Contents/MacOS/demo-mac.new"
    mv -f "$DEMO_APP/Contents/MacOS/demo-mac.new" "$DEMO_APP/Contents/MacOS/demo-mac"
    plist="$DEMO_APP/Contents/Info.plist"
    rm -f "$plist"
    plutil -create xml1 "$plist"
    plutil -insert CFBundleExecutable -string demo-mac "$plist"
    plutil -insert CFBundleIdentifier -string org.merely.scry.hardware-demo "$plist"
    plutil -insert CFBundleName -string demo-mac "$plist"
    plutil -insert CFBundleDisplayName -string "Scry demo-mac hardware gate" "$plist"
    plutil -insert CFBundlePackageType -string APPL "$plist"
    plutil -insert CFBundleShortVersionString -string 0.7.0 "$plist"
    plutil -insert CFBundleVersion -string 1 "$plist"
    plutil -insert NSHighResolutionCapable -bool true "$plist"
    codesign_identity="${SCRY_MAC_CODESIGN_IDENTITY:--}"
    codesign_args=(
        --force
        --sign "$codesign_identity"
        --identifier org.merely.scry.hardware-demo
        --timestamp=none
    )
    if [[ "$codesign_identity" = "-" ]]; then
        codesign_args+=(
            --requirements '=designated => identifier "org.merely.scry.hardware-demo"'
        )
    fi
    if ! codesign "${codesign_args[@]}" "$DEMO_APP"
    then
        exit 1
    fi
    codesign --verify --deep --strict "$DEMO_APP"
    echo "==> code-signing identity: $codesign_identity"
    codesign -d -r- "$DEMO_APP" 2>&1
    echo "==> LaunchServices app: $DEMO_APP"
fi

stop_demo_app() {
    [[ -n "$DEMO_APP" ]] || return 0
    app_executable="$DEMO_APP/Contents/MacOS/demo-mac"
    while read -r pid command; do
        if [[ "$command" = *"$app_executable"* ]]; then
            kill "$pid" 2>/dev/null || true
        fi
    done < <(ps -ax -o pid= -o command=)
    return 0
}

demo_app_running() {
    [[ -n "$DEMO_APP" ]] || return 1
    app_executable="$DEMO_APP/Contents/MacOS/demo-mac"
    while read -r _pid command; do
        if [[ "$command" = *"$app_executable"* ]]; then
            return 0
        fi
    done < <(ps -ax -o pid= -o command=)
    return 1
}
trap stop_demo_app EXIT

run_mode() {
    log="$1"
    shift
    if [[ -z "$DEMO_APP" ]]; then
        perl -e 'alarm shift; exec @ARGV' "$TIMEOUT" \
            "$DEMO_BIN" "$@" >"$log" 2>&1
        return $?
    fi

    : >"$log"
    open_args=(-n -F --stdout "$log" --stderr "$log")
    data_root="${SCRY_DEMO_DATA_ROOT:-${CARGO_TARGET_DIR:-$PWD/target}}"
    if [[ "$data_root" != /* ]]; then
        data_root="$PWD/$data_root"
    fi
    open_args+=(--env "SCRY_DEMO_DATA_ROOT=$data_root")
    # LaunchServices starts a fresh application environment. Forward the
    # graphics and diagnostic knobs that matter to the demo explicitly.
    for key in $(compgen -e); do
        case "$key" in
            MTL_* | RUST_* | SCRY_* | WGPU_* | WEBKIT_*)
                open_args+=(--env "$key=${!key}")
                ;;
        esac
    done
    stop_demo_app
    open "${open_args[@]}" "$DEMO_APP" --args "$@"
    rc=$?
    if [[ $rc -ne 0 ]]; then
        stop_demo_app
        return $rc
    fi

    # `open -W` is not reliable on every self-hosted macOS release: the app
    # can launch successfully while LaunchServices reports that it could not
    # obtain a PID. Poll the exact bundle executable and its receipt instead.
    # A completed mode always writes PASS or FAIL before exiting.
    deadline=$((SECONDS + TIMEOUT))
    launch_deadline=$((SECONDS + 5))
    saw_process=0
    foreground_control_attempted=0
    while [[ $SECONDS -lt $deadline ]]; do
        if grep -Eq 'demo-mac: .*: (PASS|FAIL)' "$log"; then
            stop_demo_app
            return 0
        fi
        if demo_app_running; then
            saw_process=1
            if [[ "$foreground_control_attempted" -eq 0 ]]; then
                foreground_control_attempted=1
                if kill -0 "$_pid" 2>/dev/null; then
                    echo "foreground-control: attempt owned-bundle pid=$_pid app=$DEMO_APP"
                    open -a "$DEMO_APP"
                    activation_rc=$?
                    echo "foreground-control: activation exit=$activation_rc pid=$_pid"
                    if [[ $activation_rc -ne 0 ]]; then
                        return "$activation_rc"
                    fi
                else
                    echo "foreground-control: owned-bundle pid=$_pid exited before activation; skipped"
                fi
            fi
        elif [[ $saw_process -eq 1 ]]; then
            return 0
        elif [[ $SECONDS -ge $launch_deadline && -s "$log" ]]; then
            # A very short-lived process can fall between polls. Its complete
            # redirected output is still enough for the caller to judge the
            # required PASS receipt.
            return 0
        fi
        sleep 0.1
    done
    stop_demo_app
    return 142
}

passed=0
failed=0
failed_modes=()

for mode in "${MODES[@]}"; do
    echo
    echo "==> $mode"
    args=("$mode")
    if [[ "$mode" = "--capture-test+resize" ]]; then
        args=(--capture-test --resize-test)
    fi
    if [[ "${CAPTURE_ACTIVITY:-0}" = "1" ]]; then
        args+=(--capture-activity)
    fi
    # The two-tabs gate is already visible by default and uses the absence of
    # --visible to retain its bounded auto-exit deadline.
    if [[ "${VISIBLE:-0}" = "1" && "$mode" != "--two-tabs" ]]; then
        args+=(--visible)
    fi
    log="${TMPDIR:-/tmp}/scry-demo-mac-$$-${mode#--}.log"
    if run_mode "$log" "${args[@]}"; then
        rc=0
    else
        rc=$?
    fi
    if [[ -f "$log" ]]; then
        cat "$log"
    fi
    if [[ $rc -eq 0 ]] && grep -q 'demo-mac: .*: PASS' "$log"; then
        echo "  -> PASS"
        passed=$((passed + 1))
    else
        if [[ $rc -eq 142 ]]; then
            echo "  -> FAIL (timed out after ${TIMEOUT}s)"
        elif [[ $rc -eq 0 ]]; then
            echo "  -> FAIL (PASS receipt absent)"
        else
            echo "  -> FAIL (exit $rc)"
        fi
        failed=$((failed + 1))
        failed_modes+=("$mode")
    fi
    rm -f "$log"
done

echo
echo "==> summary"
echo "  passed: $passed / ${#MODES[@]}"
echo "  failed: $failed"
if [[ $failed -gt 0 ]]; then
    for m in "${failed_modes[@]}"; do
        echo "    - $m"
    done
    exit 1
fi
echo "  all PASS"
