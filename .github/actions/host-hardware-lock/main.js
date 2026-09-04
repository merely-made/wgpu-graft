"use strict";

const crypto = require("crypto");
const fs = require("fs");
const os = require("os");
const path = require("path");

function inputSeconds(name, fallback) {
  const raw = process.env[`INPUT_${name.toUpperCase()}`];
  if (raw === undefined || raw === "") return fallback;
  const value = Number(raw);
  if (!Number.isFinite(value) || value < 0) {
    throw new Error(`${name} must be a non-negative number, got ${JSON.stringify(raw)}`);
  }
  return value;
}

function sleep(milliseconds) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, milliseconds);
}

function readOwner(lockDir) {
  try {
    return JSON.parse(fs.readFileSync(path.join(lockDir, "owner.json"), "utf8"));
  } catch (_) {
    try {
      return { acquired_at_ms: fs.statSync(lockDir).mtimeMs, unreadable: true };
    } catch (_) {
      return null;
    }
  }
}

function describeOwner(owner) {
  if (!owner) return "unknown owner";
  if (owner.unreadable) return "owner metadata unavailable";
  return [owner.repository, owner.workflow, owner.job, owner.run_id]
    .filter(Boolean)
    .join(" / ") || "unknown owner";
}

function saveState(name, value) {
  const stateFile = process.env.GITHUB_STATE;
  if (!stateFile) throw new Error("GITHUB_STATE is unavailable");
  fs.appendFileSync(stateFile, `${name}=${value}${os.EOL}`, "utf8");
}

function removeAbandonedLock(lockDir, token) {
  const abandoned = `${lockDir}.abandoned-${token}`;
  try {
    fs.renameSync(lockDir, abandoned);
  } catch (error) {
    if (error.code === "ENOENT" || error.code === "EACCES" || error.code === "EPERM") {
      return false;
    }
    throw error;
  }
  fs.rmSync(abandoned, { recursive: true, force: true });
  return true;
}

function main() {
  const timeoutMs = inputSeconds("timeout_seconds", 7200) * 1000;
  const staleMs = inputSeconds("stale_seconds", 14400) * 1000;
  const pollMs = Math.max(inputSeconds("poll_seconds", 5) * 1000, 100);
  if (staleMs <= timeoutMs) {
    throw new Error("stale_seconds must be greater than timeout_seconds");
  }

  const lockRoot = process.env.WGPU_HARDWARE_LOCK_ROOT ||
    path.join(os.tmpdir(), "merely-wgpu-hardware-lock");
  const lockDir = path.join(lockRoot, "host.lock");
  const token = crypto.randomUUID();
  const startedAt = Date.now();
  const owner = {
    token,
    acquired_at_ms: startedAt,
    repository: process.env.GITHUB_REPOSITORY || "local",
    workflow: process.env.GITHUB_WORKFLOW || "local",
    job: process.env.GITHUB_JOB || "local",
    run_id: process.env.GITHUB_RUN_ID || "local",
    run_attempt: process.env.GITHUB_RUN_ATTEMPT || "local",
  };

  fs.mkdirSync(lockRoot, { recursive: true });
  for (;;) {
    try {
      fs.mkdirSync(lockDir);
      try {
        fs.writeFileSync(
          path.join(lockDir, "owner.json"),
          `${JSON.stringify(owner, null, 2)}${os.EOL}`,
          { encoding: "utf8", flag: "wx" },
        );
      } catch (error) {
        fs.rmSync(lockDir, { recursive: true, force: true });
        throw error;
      }
      saveState("token", token);
      saveState("lock_dir", lockDir);
      console.log(`Acquired host hardware lock at ${lockDir}`);
      return;
    } catch (error) {
      if (error.code !== "EEXIST") throw error;
    }

    const now = Date.now();
    const current = readOwner(lockDir);
    const acquiredAt = Number(current && current.acquired_at_ms);
    if (Number.isFinite(acquiredAt) && now - acquiredAt > staleMs) {
      if (removeAbandonedLock(lockDir, token)) {
        console.warn(`Recovered stale host hardware lock from ${describeOwner(current)}`);
      }
      continue;
    }
    if (now - startedAt >= timeoutMs) {
      throw new Error(
        `Timed out waiting for host hardware lock held by ${describeOwner(current)}`,
      );
    }
    console.log(`Waiting for host hardware lock held by ${describeOwner(current)}`);
    sleep(Math.min(pollMs, timeoutMs - (now - startedAt)));
  }
}

try {
  main();
} catch (error) {
  console.error(`::error::${error.stack || error}`);
  process.exitCode = 1;
}
