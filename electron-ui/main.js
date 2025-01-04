const { app, BrowserWindow, ipcMain, dialog, shell } = require('electron');
const path = require('path');
const { spawn } = require('child_process');
const fs = require('fs').promises;

// Read version from package.json
const CURRENT_VERSION = require('./package.json').version;
const REPO_OWNER = 'mwmuni';
const REPO_NAME = 'recursive-file-content-searcher';

let mainWindow;
let currentSearchProcess = null;

// Add this function for version checking
async function checkForUpdates() {
    try {
        const response = await fetch(
            `https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}/releases/latest`,
            {
                headers: {
                    'User-Agent': 'recursive-file-content-searcher',
                    'Accept': 'application/vnd.github.v3+json'
                }
            }
        );

        if (response.ok) {
            const release = await response.json();
            const latestVersion = release.tag_name.replace(/^v/, '');
            
            // Compare versions
            const hasUpdate = require('semver').gt(latestVersion, CURRENT_VERSION);
            
            if (hasUpdate) {
                mainWindow.webContents.send('version-check', {
                    hasUpdate: true,
                    currentVersion: CURRENT_VERSION,
                    latestVersion,
                    downloadUrl: release.html_url
                });
            }
        }
    } catch (error) {
        console.error('Error checking for updates:', error);
    }
}

function createWindow() {
    mainWindow = new BrowserWindow({
        width: 1400,
        height: 720,
        minWidth: 1000,
        minHeight: 600,
        backgroundColor: '#f5f5f5',
        show: true,
        webPreferences: {
            nodeIntegration: true,
            contextIsolation: false,
            spellcheck: false,
            enableRemoteModule: false
        }
    });

    mainWindow.loadFile('index.html');
    mainWindow.once('ready-to-show', () => {
        mainWindow.show();
        // Check for updates after window is ready
        checkForUpdates();
    });
}

app.whenReady().then(createWindow);

app.on('window-all-closed', () => {
    app.quit();
});

app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
        createWindow();
    }
});

ipcMain.handle('select-directory', async () => {
    const result = await dialog.showOpenDialog(mainWindow, {
        properties: ['openDirectory']
    });
    return result.filePaths[0];
});

ipcMain.handle('show-in-folder', async (event, filePath) => {
    shell.showItemInFolder(filePath);
});

ipcMain.handle('validate-path', async (event, path) => {
    try {
        const stats = await fs.stat(path);
        return stats.isDirectory();
    } catch (err) {
        return false;
    }
});

function getBinaryPath() {
    return path.join(process.resourcesPath, 'bin/filesystem-searcher.exe');
}

ipcMain.handle('search-files', async (event, { pattern, directory, sizeLimit, filePattern }) => {
    return new Promise((resolve, reject) => {
        const args = [pattern, directory, sizeLimit.toString()];
        if (filePattern) {
            args.push(filePattern);
        }
        
        const searcher = spawn(getBinaryPath(), args);
        currentSearchProcess = searcher;

        // Remove output accumulation
        // let output = ''; // Removed
        let error = '';

        searcher.stdout.on('data', (data) => {
            const text = data.toString();
            // output += text; // Removed
            mainWindow.webContents.send('search-progress', text);
        });

        searcher.stderr.on('data', (data) => {
            error += data.toString();
        });

        searcher.on('close', (code) => {
            currentSearchProcess = null;
            if (code === 0) {
                resolve(); // Resolve without output
            } else {
                reject(error || 'Search process failed');
            }
        });

        searcher.on('error', (err) => {
            currentSearchProcess = null;
            reject(`Failed to start search process: ${err.message}`);
        });
    });
});

ipcMain.handle('stop-search', async () => {
    if (currentSearchProcess) {
        try {
            currentSearchProcess.kill();
            currentSearchProcess = null;
            return true;
        } catch (error) {
            console.error('Error stopping search:', error);
            return false;
        }
    }
    return false;
});
