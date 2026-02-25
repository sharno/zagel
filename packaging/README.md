# Packaging

This directory is the source of truth for package-manager manifests.

- `homebrew/zagel.rb`: Homebrew formula for a dedicated tap.
- `scoop/zagel.json`: Scoop manifest for a dedicated bucket.
- `winget/manifests/...`: Winget manifests matching winget-pkgs layout.
- `chocolatey/`: Chocolatey nuspec and install scripts.
- `aur/PKGBUILD`: AUR `zagel-bin` package spec.

## Update flow

1. Publish a GitHub release (`vX.Y.Z`).
2. Run `packaging-update.yml` (or wait for release trigger).
3. The workflow opens a PR updating manifests in this directory.
4. Submit the updated files to each upstream package-manager repository.
