"use strict";

const fs = require("fs");
const path = require("path");

function main() {
  const token = process.env.STATE_token;
  const lockDir = process.env.STATE_lock_dir;
  if (!token || !lockDir) return;

  let owner;
  try {
    owner = JSON.parse(fs.readFileSync(path.join(lockDir, "owner.json"), "utf8"));
  } catch (error) {
    if (error.code === "ENOENT") return;
    throw error;
  }
  if (owner.token !== token) {
    console.warn("Host hardware lock belongs to a newer job; leaving it intact");
    return;
  }
  fs.rmSync(lockDir, { recursive: true, force: true });
  console.log(`Released host hardware lock at ${lockDir}`);
}

try {
  main();
} catch (error) {
  console.error(`::warning::Failed to release host hardware lock: ${error.stack || error}`);
  process.exitCode = 1;
}
