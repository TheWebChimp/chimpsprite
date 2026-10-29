**Alpha** — early preview of ChimpSprite, a pixel-art and sprite editor written in Rust. Expect bugs and breaking changes; keep backups of your work.

Try it in the browser without installing: https://thewebchimp.github.io/chimpsprite/

## Downloads

| System | File |
| --- | --- |
| Windows 10/11 (64-bit) | `ChimpSprite_*_x64-setup.exe` |
| macOS, Apple Silicon (M1 and newer) | `ChimpSprite_*_aarch64.dmg` |
| macOS, Intel | `ChimpSprite_*_x64.dmg` |
| Linux (Debian/Ubuntu) | `chimpsprite_*_amd64.deb` |
| Linux (any distro) | `chimpsprite_*_x86_64.AppImage` |

## The installers are not signed

- **Windows:** SmartScreen may say "Windows protected your PC". Click **More info → Run anyway**.
- **macOS:** drag ChimpSprite to Applications. If macOS says the app "is damaged" or "can't be opened", run this once in Terminal:
  ```
  xattr -dr com.apple.quarantine /Applications/ChimpSprite.app
  ```
- **Linux AppImage:** make it executable (`chmod +x chimpsprite_*.AppImage`) and run it.
