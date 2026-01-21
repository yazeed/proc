#!/usr/bin/env node

const https = require("https");
const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");
const zlib = require("zlib");

const VERSION = require("./package.json").version;
const REPO = "yazeed/proc";

function getPlatformInfo() {
  const platform = process.platform;
  const arch = process.arch;

  let os, archName, ext, archiveExt;

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

function download(url) {
  return new Promise((resolve, reject) => {
    https
      .get(url, (response) => {
        if (response.statusCode === 302 || response.statusCode === 301) {
          download(response.headers.location).then(resolve).catch(reject);
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

async function extractTarGz(buffer, destDir, binaryName) {
  const tar = require("tar");
  const tmpFile = path.join(destDir, "tmp.tar.gz");
  fs.writeFileSync(tmpFile, buffer);

  await tar.extract({
    file: tmpFile,
    cwd: destDir,
    filter: (p) => p.includes(binaryName.replace(".exe", "")),
  });

  fs.unlinkSync(tmpFile);
}

async function extractZip(buffer, destDir, binaryName) {
  const AdmZip = require("adm-zip");
  const zip = new AdmZip(buffer);
  zip.extractAllTo(destDir, true);
}

async function main() {
  const { binaryName, archiveName, platform } = getPlatformInfo();
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${archiveName}`;
  const binDir = __dirname;
  const binPath = path.join(binDir, platform === "win32" ? "proc.exe" : "proc");

  console.log(`Downloading proc v${VERSION}...`);
  console.log(`  URL: ${url}`);

  try {
    const buffer = await download(url);

    if (archiveName.endsWith(".tar.gz")) {
      // Simple tar.gz extraction without external dependency
      const tmpFile = path.join(binDir, "tmp.tar.gz");
      fs.writeFileSync(tmpFile, buffer);

      // Use system tar
      execSync(`tar -xzf "${tmpFile}" -C "${binDir}"`, { stdio: "pipe" });
      fs.unlinkSync(tmpFile);

      // Rename extracted binary
      const extractedBin = path.join(binDir, binaryName);
      if (fs.existsSync(extractedBin)) {
        fs.renameSync(extractedBin, binPath);
      }
    } else if (archiveName.endsWith(".zip")) {
      // Use system unzip on Windows
      const tmpFile = path.join(binDir, "tmp.zip");
      fs.writeFileSync(tmpFile, buffer);

      execSync(`powershell -Command "Expand-Archive -Force '${tmpFile}' '${binDir}'"`, {
        stdio: "pipe",
      });
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
    console.error("You can install manually from: https://github.com/yazeed/proc/releases");
    process.exit(1);
  }
}

main();
