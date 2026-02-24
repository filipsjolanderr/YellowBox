import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import AdmZip from 'adm-zip';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const binDir = path.join(__dirname, '..', 'src-tauri', 'bin');

if (!fs.existsSync(binDir)) {
    fs.mkdirSync(binDir, { recursive: true });
}

const downloads = [
    {
        name: 'ffmpeg-x86_64-pc-windows-msvc.exe',
        url: 'https://github.com/ffbinaries/ffbinaries-prebuilt/releases/download/v6.1/ffmpeg-6.1-win-64.zip',
        zipEntry: 'ffmpeg.exe'
    },
    {
        name: 'ffmpeg-aarch64-pc-windows-msvc.exe',
        url: 'https://github.com/ffbinaries/ffbinaries-prebuilt/releases/download/v6.1/ffmpeg-6.1-win-64.zip',
        zipEntry: 'ffmpeg.exe'
    },
    {
        name: 'ffmpeg-x86_64-apple-darwin',
        url: 'https://github.com/ffbinaries/ffbinaries-prebuilt/releases/download/v6.1/ffmpeg-6.1-macos-64.zip',
        zipEntry: 'ffmpeg'
    },
    {
        name: 'ffmpeg-aarch64-apple-darwin',
        url: 'https://github.com/ffbinaries/ffbinaries-prebuilt/releases/download/v6.1/ffmpeg-6.1-macos-64.zip',
        zipEntry: 'ffmpeg'
    },
    {
        name: 'ffmpeg-x86_64-unknown-linux-gnu',
        url: 'https://github.com/ffbinaries/ffbinaries-prebuilt/releases/download/v6.1/ffmpeg-6.1-linux-64.zip',
        zipEntry: 'ffmpeg'
    },
    {
        name: 'ffmpeg-aarch64-unknown-linux-gnu',
        url: 'https://github.com/ffbinaries/ffbinaries-prebuilt/releases/download/v6.1/ffmpeg-6.1-linux-arm-64.zip',
        zipEntry: 'ffmpeg'
    }
];

async function downloadAndExtract() {
    console.log(`Setting up FFmpeg binaries in ${binDir}`);
    for (const item of downloads) {
        const outPath = path.join(binDir, item.name);
        if (fs.existsSync(outPath)) {
            console.log(`✅ Skipping ${item.name}, already exists.`);
            continue;
        }

        console.log(`Downloading ${item.url}...`);
        const response = await fetch(item.url);
        if (!response.ok) {
            console.error(`❌ Failed to fetch ${item.url}: ${response.statusText}`);
            continue;
        }

        const arrayBuffer = await response.arrayBuffer();
        const buffer = Buffer.from(arrayBuffer);

        console.log(`Extracting ${item.zipEntry} -> ${item.name}...`);
        const zip = new AdmZip(buffer);
        const zipEntry = zip.getEntry(item.zipEntry);

        if (zipEntry) {
            fs.writeFileSync(outPath, zipEntry.getData());
            // Make executable on unix
            if (!item.name.endsWith('.exe')) {
                fs.chmodSync(outPath, 0o755);
            }
            console.log(`✅ Success: ${item.name}`);
        } else {
            console.error(`❌ Failed: Could not find ${item.zipEntry} in zip`);
        }
    }
    console.log("Done configuring FFmpeg for Tauri cross-platform compilation.");
}

downloadAndExtract().catch(console.error);
