# Filesystem Searcher

A fast, cross-platform tool for searching file contents using regular expressions. Built with Rust and Electron.

## Features

- Fast regex-based file content search
- Modern, intuitive user interface
- Configurable file size limits
- Real-time search results
- Click to open containing folders
- Multi-threaded search for optimal performance

## Prerequisites

- Rust (1.70 or later)
- Node.js (16.0 or later)
- npm (8.0 or later)

## Installation

### Windows
```bash
# Run the build script
.\build.bat
```

### Linux/macOS
```bash
# Run the build script
./build.sh
```

Or build manually:

1. Build the Rust binary:
```bash
cargo build --release
```

2. Build the Electron UI:
```bash
cd electron-ui
npm install
npm run build
```

## Usage

After building, you can find the application in:
- Windows: `electron-ui/dist/win-unpacked/Filesystem Searcher.exe`
- Linux: `electron-ui/dist/linux-unpacked/filesystem-searcher`
- macOS: `electron-ui/dist/mac/Filesystem Searcher.app`

### Search Options

1. **Search Pattern**: Enter a regular expression pattern to search for
2. **Search Directory**: Select the directory to search in
3. **File Size Limit**: Set maximum file size to search (in MB, default 1000)

### Tips

- Use the file list on the right to quickly navigate to found files
- Click any file in the list to open its containing folder
- The search is performed in real-time with progress indication
- Large files are automatically skipped based on the size limit

## Command Line Usage

The Rust binary can also be used standalone:

```bash
filesystem-searcher <regex_pattern> [starting_path] [size_limit_mb]
```

Example:
```bash
filesystem-searcher "TODO:" ./src 100
```

## Development

- Rust source is in `src/`
- Electron UI source is in `electron-ui/`
- Build outputs go to `target/` and `electron-ui/dist/`
