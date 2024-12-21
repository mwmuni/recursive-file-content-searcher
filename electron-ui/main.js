const { app, BrowserWindow, ipcMain, dialog, shell } = require('electron');
const path = require('path');
const { spawn } = require('child_process');

let mainWindow;

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

function getBinaryPath() {
    return path.join(process.resourcesPath, 'bin/filesystem-searcher.exe');
}

ipcMain.handle('search-files', async (event, { pattern, directory, sizeLimit }) => {
    return new Promise((resolve, reject) => {
        const searcher = spawn(getBinaryPath(), [pattern, directory, sizeLimit.toString()]);

        let output = '';
        let error = '';

        searcher.stdout.on('data', (data) => {
            const text = data.toString();
            output += text;
            mainWindow.webContents.send('search-progress', text);
        });

        searcher.stderr.on('data', (data) => {
            error += data.toString();
        });

        searcher.on('close', (code) => {
            if (code === 0) {
                resolve(output);
            } else {
                reject(error || 'Search process failed');
            }
        });

        searcher.on('error', (err) => {
            reject(`Failed to start search process: ${err.message}`);
        });
    });
}); 