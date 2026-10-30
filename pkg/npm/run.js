#!/usr/bin/env node

const { spawn } = require("child_process");
const fs = require("fs");
const os = require("os");
const { install, binaryPath, cacheDir } = require("./install.js");

/**
 * Find the proc binary, installing it on first use if needed.
 *
 * The package has no install script, so the first run downloads the binary.
 * Prefer the package directory; fall back to a per-user cache directory when
 * the package directory is not writable.
 */
async function resolveBinary() {
  for (const dir of [__dirname, cacheDir()]) {
    const candidate = binaryPath(dir);
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }

  try {
    return await install(__dirname);
  } catch (err) {
    if (err.code === "EACCES" || err.code === "EPERM" || err.code === "EROFS") {
      return install(cacheDir());
    }
    throw err;
  }
}

resolveBinary()
  .then((binPath) => {
    const child = spawn(binPath, process.argv.slice(2), {
      stdio: "inherit",
    });

    child.on("error", (err) => {
      console.error(`Failed to run proc: ${err.message}`);
      process.exit(1);
    });

    child.on("exit", (code, signal) => {
      if (signal) {
        // End by the same signal, as proc did, so a shell knows a signal
        // ended it (Ctrl-C stops a script's loop); 128 plus its number if
        // this process survives it
        process.kill(process.pid, signal);
        process.exit(128 + (os.constants.signals[signal] || 0));
      }
      process.exit(code);
    });
  })
  .catch((err) => {
    console.error(`Failed to install proc: ${err.message}`);
    console.error(
      "You can install manually from: https://github.com/yazeed/proc-cli/releases"
    );
    process.exit(1);
  });
