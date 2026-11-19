"use strict";

const { spawn, execFileSync } = require("node:child_process");
const fs = require("node:fs");
const https = require("node:https");
const os = require("node:os");
const path = require("node:path");

const pkg = require("../package.json");
const REPO = "ptai-eng/chronos-agent";

function resolvePlatformAsset() {
  const key = `${process.platform}:${process.arch}`;
  switch (key) {
    case "win32:x64":
      return { archive: "chronos-windows-x64.zip", ext: ".exe", kind: "zip" };
    case "linux:x64":
      return { archive: "chronos-linux-x64.tar.gz", ext: "", kind: "tgz" };
    case "darwin:arm64":
      return { archive: "chronos-macos-arm64.tar.gz", ext: "", kind: "tgz" };
    default:
      throw new Error(
        `Unsupported platform '${key}'. Prebuilt archives support win32-x64, linux-x64, and darwin-arm64. Build from source with: cargo build --release`
      );
  }
}

function resolveCacheRoot() {
  if (process.env.CHRONOS_CACHE_DIR && process.env.CHRONOS_CACHE_DIR.trim() !== "") {
    return process.env.CHRONOS_CACHE_DIR.trim();
  }
  return path.join(os.homedir(), ".cache", "chronos-agent");
}

function downloadFile(url, destPath, redirectsLeft = 6) {
  return new Promise((resolve, reject) => {
    const req = https.get(
      url,
      { headers: { "User-Agent": `chronos-mcp-npx/${pkg.version}` } },
      (res) => {
        if (
          res.statusCode >= 300 &&
          res.statusCode < 400 &&
          res.headers.location &&
          redirectsLeft > 0
        ) {
          res.resume();
          const nextUrl = new URL(res.headers.location, url).toString();
          downloadFile(nextUrl, destPath, redirectsLeft - 1).then(resolve, reject);
          return;
        }
        if (res.statusCode !== 200) {
          res.resume();
          reject(new Error(`HTTP ${res.statusCode} while downloading ${url}`));
          return;
        }
        const file = fs.createWriteStream(destPath);
        res.pipe(file);
        file.on("finish", () => file.close(resolve));
        file.on("error", (err) => {
          fs.rmSync(destPath, { force: true });
          reject(err);
        });
      }
    );
    req.on("error", reject);
  });
}

function extractArchive(archivePath, destDir, kind) {
  fs.mkdirSync(destDir, { recursive: true });
  if (kind === "zip") {
    execFileSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        `Expand-Archive -Path '${archivePath.replace(/'/g, "''")}' -DestinationPath '${destDir.replace(/'/g, "''")}' -Force`,
      ],
      { stdio: "ignore" }
    );
  } else {
    execFileSync("tar", ["-xzf", archivePath, "-C", destDir], { stdio: "ignore" });
  }
}

async function ensureBinary(binaryBaseName) {
  const versionTag = process.env.CHRONOS_VERSION || `v${pkg.version}`;
  const asset = resolvePlatformAsset();
  const installDir = path.join(
    resolveCacheRoot(),
    `${versionTag}-${process.platform}-${process.arch}`
  );
  const targetExe = path.join(installDir, `${binaryBaseName}${asset.ext}`);

  if (fs.existsSync(targetExe)) {
    return targetExe;
  }

  fs.mkdirSync(path.dirname(installDir), { recursive: true });
  const stageDir = fs.mkdtempSync(path.join(os.tmpdir(), "chronos-npx-"));
  try {
    const archivePath = path.join(stageDir, asset.archive);
    const extractDir = path.join(stageDir, "bin");
    const downloadUrl = `https://github.com/${REPO}/releases/download/${versionTag}/${asset.archive}`;
    await downloadFile(downloadUrl, archivePath);
    extractArchive(archivePath, extractDir, asset.kind);

    if (!fs.existsSync(path.join(extractDir, `${binaryBaseName}${asset.ext}`))) {
      throw new Error(`Expected binary ${binaryBaseName}${asset.ext} missing after extraction.`);
    }

    // Atomic move into final cache dir (tolerant of concurrent invocations)
    if (!fs.existsSync(installDir)) {
      fs.renameSync(extractDir, installDir);
    }

    if (process.platform !== "win32") {
      for (const bin of ["chronos", "chronos-mcp"]) {
        const p = path.join(installDir, bin);
        if (fs.existsSync(p)) {
          fs.chmodSync(p, 0o755);
        }
      }
    }
    return targetExe;
  } finally {
    fs.rmSync(stageDir, { recursive: true, force: true });
  }
}

async function runBinary(binaryBaseName, args) {
  const exePath = await ensureBinary(binaryBaseName);
  const child = spawn(exePath, args, { stdio: "inherit" });
  child.on("error", (err) => {
    console.error(`[chronos-mcp] Failed to launch '${exePath}': ${err.message}`);
    process.exit(1);
  });
  for (const sig of ["SIGINT", "SIGTERM"]) {
    process.on(sig, () => {
      if (!child.killed) child.kill(sig);
    });
  }
  child.on("exit", (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
    } else {
      process.exit(code ?? 0);
    }
  });
}

module.exports = { ensureBinary, runBinary, resolvePlatformAsset };
