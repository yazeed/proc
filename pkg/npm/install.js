#!/usr/bin/env node

const https = require("https");
const fs = require("fs");
const path = require("path");
const crypto = require("crypto");
const { execFileSync } = require("child_process");

const VERSION = require("./package.json").version;
const REPO = "yazeed/proc";

// Only follow redirects to trusted GitHub domains
const ALLOWED_HOSTS = [
  "github.com",
  "objects.githubusercontent.com",
  "github-releases.githubusercontent.com",
];
const MAX_REDIRECTS = 5;

function getPlatformInfo() {
  const platform = process.platform;
  const arch = process.arch;

  let os, archName, archiveExt;

  switch (platform) {
    case "darwin":
      os = "darwin";
      archiveExt = ".tar.gz";
      break;
    case "linux":
      os = "linux";
      archiveExt = ".tar.gz";
      break;
    case "win32":
      os = "windows";
      archiveExt = ".exe.zip";
      break;
    default:
      throw new Error(`Unsupported platform: ${platform}`);
  }

  switch (arch) {
    case "x64":
      archName = "x86_64";
      break;
    case "arm64":
      archName = "aarch64";
      break;
    default:
      throw new Error(`Unsupported architecture: ${arch}`);
  }

  const binaryName = `proc-${os}-${archName}${platform === "win32" ? ".exe" : ""}`;
  const archiveName = `proc-${os}-${archName}${archiveExt}`;

  return { os, archName, binaryName, archiveName, platform };
}

function download(url, redirectDepth = 0) {
  return new Promise((resolve, reject) => {
    if (redirectDepth > MAX_REDIRECTS) {
      return reject(new Error("Too many redirects"));
    }

    const parsed = new URL(url);
    if (!ALLOWED_HOSTS.includes(parsed.hostname)) {
      return reject(
        new Error(`Redirect to untrusted host: ${parsed.hostname}`)
      );
    }

    https
      .get(url, (response) => {
        if (response.statusCode === 302 || response.statusCode === 301) {
          download(response.headers.location, redirectDepth + 1)
            .then(resolve)
            .catch(reject);
          return;
        }
        if (response.statusCode !== 200) {
          reject(new Error(`HTTP ${response.statusCode}`));
          return;
        }
        const chunks = [];
        response.on("data", (chunk) => chunks.push(chunk));
        response.on("end", () => resolve(Buffer.concat(chunks)));
        response.on("error", reject);
      })
      .on("error", reject);
  });
}

function verifyChecksum(buffer, expectedHash, archiveName) {
  const actualHash = crypto.createHash("sha256").update(buffer).digest("hex");
  if (actualHash !== expectedHash) {
    throw new Error(
      `Checksum mismatch for ${archiveName}!\n` +
        `  Expected: ${expectedHash}\n` +
        `  Actual:   ${actualHash}\n` +
        `  The download may be corrupted or tampered with.`
    );
  }
}

function parseChecksums(raw, archiveName) {
  const lines = raw.toString().split("\n");
  for (const line of lines) {
    // Format: "hash  filename" (sha256sum output)
    const parts = line.trim().split(/\s+/);
    if (parts.length >= 2 && parts[1] === archiveName) {
      return parts[0];
    }
  }
  throw new Error(
    `Checksum not found for ${archiveName} in checksums.txt`
  );
}

async function main() {
  const { binaryName, archiveName, platform } = getPlatformInfo();
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${archiveName}`;
  const checksumsUrl = `https://github.com/${REPO}/releases/download/v${VERSION}/checksums.txt`;
  const binDir = __dirname;
  const binPath = path.join(binDir, platform === "win32" ? "proc.exe" : "proc");

  console.log(`Downloading proc v${VERSION}...`);
  console.log(`  URL: ${url}`);

  try {
    // Download checksums and archive in parallel
    const [checksumsRaw, buffer] = await Promise.all([
      download(checksumsUrl),
      download(url),
    ]);

    // Verify integrity before extracting
    const expectedHash = parseChecksums(checksumsRaw, archiveName);
    verifyChecksum(buffer, expectedHash, archiveName);
    console.log(`  Checksum verified: ${expectedHash.slice(0, 16)}...`);

    if (archiveName.endsWith(".tar.gz")) {
      const tmpFile = path.join(binDir, "tmp.tar.gz");
      fs.writeFileSync(tmpFile, buffer);

      // Use execFileSync with argument array — no shell injection possible
      execFileSync("tar", ["-xzf", tmpFile, "-C", binDir], {
        stdio: "pipe",
      });
      fs.unlinkSync(tmpFile);

      // Rename extracted binary
      const extractedBin = path.join(binDir, binaryName);
      if (fs.existsSync(extractedBin)) {
        fs.renameSync(extractedBin, binPath);
      }
    } else if (archiveName.endsWith(".zip")) {
      const tmpFile = path.join(binDir, "tmp.zip");
      fs.writeFileSync(tmpFile, buffer);

      // Use execFileSync with argument array — no shell injection possible
      execFileSync(
        "powershell",
        [
          "-Command",
          `Expand-Archive -Force -Path '${tmpFile.replace(/'/g, "''")}' -DestinationPath '${binDir.replace(/'/g, "''")}'`,
        ],
        { stdio: "pipe" }
      );
      fs.unlinkSync(tmpFile);

      // Rename extracted binary
      const extractedBin = path.join(binDir, binaryName);
      if (fs.existsSync(extractedBin)) {
        fs.renameSync(extractedBin, binPath);
      }
    }

    // Make executable
    if (platform !== "win32") {
      fs.chmodSync(binPath, 0o755);
    }

    console.log(`  Installed to: ${binPath}`);
    console.log("Done!");
  } catch (err) {
    console.error(`Failed to install proc: ${err.message}`);
    console.error(
      "You can install manually from: https://github.com/yazeed/proc/releases"
    );
    process.exit(1);
  }
}

main();
