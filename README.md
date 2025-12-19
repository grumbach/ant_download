# Ant Download

Download files from the Autonomi Network — one click to eternal access.

Censorship-proof, universally available, and free for everyone.

Download files that were uploaded to the network forever.

Liberate the world's knowledge — access it from anywhere

![](./assets/recursion.png)

## Download it

[Download the latest release on github with a click!](https://github.com/grumbach/ant_download/releases/latest) 

Or download directly from the Autonomi Network:

```bash
# macOS aarch64
ant file download 1ac07d2e628cf7c2f208edb99d77ad928f5709d4fc5151d6d77311212e261de8 AntDownload-aarch64-apple-darwin.zip

# Linux aarch64
ant file download 22433cb165acc7716472c2ca5e1944854ab593cd490dfc585f064aa6b7fb3005 AntDownload-aarch64-unknown-linux-musl.zip

# macOS x86_64 
ant file download 0353ca609d671358220ec94b0b04c8dfde8a1a6921f11cb273354273b93392d7 AntDownload-x86_64-apple-darwin.zip

# Linux x86_64
ant file download 60d6440bdaf3028ae1e67a76e1cd10b3003b7030bd11295edf0e6ff97ceb57a7 AntDownload-x86_64-unknown-linux-musl.zip
```

*AntDownload was uploaded to the Network using [AntUpload](https://github.com/grumbach/ant_upload)!*

> Mac users might face quarantine issues: `"AntDownload.app" is damaged and can't be opened. You should move it to the Trash.`
>
> This happens because we don't have a $99 a year Apple Developer account :(
>
> To fix this:
> 1. **Unzip** the file (double-click the `.zip`).
> 2. Open **Terminal** (press `Cmd + Space`, type "Terminal", and press Enter).
> 3. Go to your Downloads folder:
>   ```bash
>   cd ~/Downloads
>   ```
> 4. Remove macOS quarantine flag:
>   ```bash
>   xattr -rd com.apple.quarantine AntDownload.app
>   ```
> 5. Double-click **AntDownload.app** to open it!

## Build it from source

```bash
# build the release version of the app
cargo build --release

# (for macOS) make a AntDownload.app
bash ./assets/mac_os_bundle.sh
```

## Run it from source

```bash
cargo run --release
```

## For those diving into the code

- The `src/server.rs` file contains the main logic for all autonomi network interaction
- The `src/main.rs` contains the GUI front-end for the app (100% vibecoded)

## Features

- Download files from the Autonomi Network using file addresses
- Simple copy-paste interface
- Cross-platform support (Linux, macOS)
- Free downloads forever

## Coming soon

- Retry failed downloads
- Download history
- Resume downloads on app restart
- Suggest more features by submitting or upvoting an issue on github