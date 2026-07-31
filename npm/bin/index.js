#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const https = require('https');
const { spawnSync, execSync } = require('child_process');

const VERSION = "1.2.3";
const REPO = "nihmadev/fs-mcp-rs";

function getPlatformInfo() {
  const platform = process.platform;
  const arch = process.arch;

  if (platform === 'win32' && arch === 'x64') {
    return {
      archiveName: `fs-mcp-rs-windows-x64-v${VERSION}.zip`,
      binName: 'fs-mcp-rs.exe',
      format: 'zip'
    };
  } else if (platform === 'linux' && arch === 'x64') {
    return {
      archiveName: `fs-mcp-rs-linux-x64-v${VERSION}.tar.gz`,
      binName: 'fs-mcp-rs',
      format: 'tar.gz'
    };
  } else if (platform === 'darwin' && arch === 'arm64') {
    return {
      archiveName: `fs-mcp-rs-macos-arm64-v${VERSION}.tar.gz`,
      binName: 'fs-mcp-rs',
      format: 'tar.gz'
    };
  } else if (platform === 'darwin' && arch === 'x64') {
    return {
      archiveName: `fs-mcp-rs-macos-x64-v${VERSION}.tar.gz`,
      binName: 'fs-mcp-rs',
      format: 'tar.gz'
    };
  }

  throw new Error(`Unsupported platform/architecture: ${platform}-${arch}`);
}

function downloadFile(url, destPath) {
  return new Promise((resolve, reject) => {
    const request = (currentUrl) => {
      https.get(currentUrl, (response) => {
        if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
          return request(response.headers.location);
        }
        if (response.statusCode !== 200) {
          return reject(new Error(`Failed to download ${currentUrl}: HTTP ${response.statusCode}`));
        }

        const fileStream = fs.createWriteStream(destPath);
        response.pipe(fileStream);
        fileStream.on('finish', () => {
          fileStream.close();
          resolve();
        });
        fileStream.on('error', (err) => {
          fs.unlink(destPath, () => {});
          reject(err);
        });
      }).on('error', reject);
    };
    request(url);
  });
}

function extractArchive(archivePath, extractDir, format, platform) {
  fs.mkdirSync(extractDir, { recursive: true });

  if (format === 'zip') {
    if (platform === 'win32') {
      execSync(`powershell -Command "Expand-Archive -Path '${archivePath}' -DestinationPath '${extractDir}' -Force"`);
    } else {
      execSync(`unzip -o "${archivePath}" -d "${extractDir}"`);
    }
  } else {
    execSync(`tar -xzf "${archivePath}" -C "${extractDir}"`);
  }
}

async function ensureBinary() {
  const info = getPlatformInfo();
  const cacheDir = path.join(__dirname, '..', 'bin-cache');
  const archivePath = path.join(cacheDir, info.archiveName);
  const extractDir = path.join(cacheDir, 'extracted');
  const binaryPath = path.join(extractDir, `fs-mcp-rs-${process.platform === 'win32' ? 'windows-x64' : process.platform === 'darwin' ? (process.arch === 'arm64' ? 'macos-arm64' : 'macos-x64') : 'linux-x64'}-v${VERSION}`, info.binName);

  if (fs.existsSync(binaryPath)) {
    return binaryPath;
  }

  fs.mkdirSync(cacheDir, { recursive: true });

  const downloadUrl = `https://github.com/${REPO}/releases/download/v${VERSION}/${info.archiveName}`;
  console.log(`[fs-mcp-rs] Downloading precompiled binary from ${downloadUrl}...`);

  try {
    await downloadFile(downloadUrl, archivePath);
    console.log(`[fs-mcp-rs] Extracting binary...`);
    extractArchive(archivePath, extractDir, info.format, process.platform);

    if (process.platform !== 'win32' && fs.existsSync(binaryPath)) {
      fs.chmodSync(binaryPath, 0o755);
    }

    if (fs.existsSync(archivePath)) {
      fs.unlinkSync(archivePath);
    }

    return binaryPath;
  } catch (err) {
    console.error(`[fs-mcp-rs] Failed to setup binary:`, err.message);
    process.exit(1);
  }
}

async function main() {
  const isDownloadOnly = process.argv.includes('--download-only');

  try {
    const binaryPath = await ensureBinary();

    if (isDownloadOnly) {
      console.log(`[fs-mcp-rs] Binary ready at ${binaryPath}`);
      return;
    }

    const args = process.argv.slice(2);
    const result = spawnSync(binaryPath, args, { stdio: 'inherit' });
    process.exit(result.status ?? 0);
  } catch (err) {
    console.error(`[fs-mcp-rs] Error:`, err.message);
    process.exit(1);
  }
}

main();
