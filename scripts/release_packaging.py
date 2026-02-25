#!/usr/bin/env python3
"""Generate package manager manifests from GitHub release metadata."""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path
import textwrap

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python < 3.11 fallback
    import tomli as tomllib  # type: ignore[no-redef]


REPO_ROOT = Path(__file__).resolve().parent.parent


@dataclass(frozen=True)
class ReleaseAsset:
    name: str
    url: str
    sha256: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--release-json",
        type=Path,
        required=True,
        help="Path to `gh release view --json tagName,assets` output",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("packaging"),
        help="Directory to write manifests into (default: packaging)",
    )
    return parser.parse_args()


def read_cargo_metadata() -> dict[str, str]:
    cargo_toml = REPO_ROOT / "Cargo.toml"
    if not cargo_toml.is_file():
        raise FileNotFoundError(f"could not find Cargo.toml at {cargo_toml}")

    cargo_data = tomllib.loads(cargo_toml.read_text(encoding="utf-8"))
    package = cargo_data["package"]
    return {
        "name": package["name"],
        "description": package["description"],
        "license": package["license"],
        "repository": package["repository"],
    }


def parse_sha256(digest: str, asset_name: str) -> str:
    prefix = "sha256:"
    if not digest.startswith(prefix):
        raise ValueError(f"asset {asset_name} is missing a sha256 digest")
    return digest[len(prefix) :]


def load_assets(release_json: Path) -> tuple[str, list[ReleaseAsset]]:
    payload = json.loads(release_json.read_text(encoding="utf-8"))
    tag = payload["tagName"]
    assets = [
        ReleaseAsset(
            name=asset["name"],
            url=asset["url"],
            sha256=parse_sha256(asset.get("digest", ""), asset["name"]),
        )
        for asset in payload["assets"]
    ]
    return tag, assets


def find_asset(assets: list[ReleaseAsset], expected_suffix: str) -> ReleaseAsset:
    matches = [asset for asset in assets if asset.name.endswith(expected_suffix)]
    if not matches:
        raise ValueError(f"could not find release asset ending in '{expected_suffix}'")
    if len(matches) > 1:
        raise ValueError(f"found multiple assets ending in '{expected_suffix}'")
    return matches[0]


def write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def build_homebrew_formula(
    version: str,
    metadata: dict[str, str],
    linux_asset: ReleaseAsset,
    mac_x64_asset: ReleaseAsset,
    mac_arm_asset: ReleaseAsset,
) -> str:
    return textwrap.dedent(
        f"""\
        class Zagel < Formula
          desc \"{metadata["description"]}\"
          homepage \"{metadata["repository"]}\"
          version \"{version}\"
          license \"{metadata["license"]}\"

          on_macos do
            if Hardware::CPU.arm?
              url \"{mac_arm_asset.url}\"
              sha256 \"{mac_arm_asset.sha256}\"
            end

            if Hardware::CPU.intel?
              url \"{mac_x64_asset.url}\"
              sha256 \"{mac_x64_asset.sha256}\"
            end
          end

          on_linux do
            if Hardware::CPU.intel?
              url \"{linux_asset.url}\"
              sha256 \"{linux_asset.sha256}\"
            else
              odie \"zagel currently supports x86_64 Linux only\"
            end
          end

          def install
            bin.install \"zagel\"
          end

          test do
            assert_predicate bin/\"zagel\", :exist?
          end
        end
        """
    )


def build_scoop_manifest(
    version: str,
    metadata: dict[str, str],
    windows_asset: ReleaseAsset,
) -> str:
    manifest = {
        "version": version,
        "description": metadata["description"],
        "homepage": metadata["repository"],
        "license": metadata["license"],
        "architecture": {
            "64bit": {
                "url": windows_asset.url,
                "hash": windows_asset.sha256,
            }
        },
        "bin": "zagel.exe",
        "checkver": "github",
        "autoupdate": {
            "architecture": {
                "64bit": {
                    "url": (
                        f"{metadata['repository']}/releases/download/v$version/"
                        "zagel-v$version-x86_64-pc-windows-msvc.zip"
                    )
                }
            }
        },
    }
    return json.dumps(manifest, indent=2) + "\n"


def build_winget_version_manifest(version: str) -> str:
    return textwrap.dedent(
        f"""\
        PackageIdentifier: Sharno.Zagel
        PackageVersion: {version}
        DefaultLocale: en-US
        ManifestType: version
        ManifestVersion: 1.9.0
        """
    )


def build_winget_installer_manifest(version: str, windows_asset: ReleaseAsset) -> str:
    return textwrap.dedent(
        f"""\
        PackageIdentifier: Sharno.Zagel
        PackageVersion: {version}
        Installers:
          - Architecture: x64
            InstallerType: zip
            NestedInstallerType: portable
            NestedInstallerFiles:
              - RelativeFilePath: zagel.exe
                PortableCommandAlias: zagel
            InstallerUrl: {windows_asset.url}
            InstallerSha256: {windows_asset.sha256}
        ManifestType: installer
        ManifestVersion: 1.9.0
        """
    )


def build_winget_locale_manifest(version: str, metadata: dict[str, str]) -> str:
    return textwrap.dedent(
        f"""\
        PackageIdentifier: Sharno.Zagel
        PackageVersion: {version}
        PackageLocale: en-US
        Publisher: sharno
        PublisherUrl: https://github.com/sharno
        PublisherSupportUrl: {metadata["repository"]}/issues
        Author: sharno
        PackageName: Zagel
        PackageUrl: {metadata["repository"]}
        License: {metadata["license"]}
        ShortDescription: {metadata["description"]}
        Moniker: zagel
        Tags:
          - rest
          - http
          - api
          - client
        ManifestType: defaultLocale
        ManifestVersion: 1.9.0
        """
    )


def build_choco_nuspec(version: str, metadata: dict[str, str]) -> str:
    return textwrap.dedent(
        f"""\
        <?xml version=\"1.0\"?>
        <package xmlns=\"http://schemas.microsoft.com/packaging/2015/06/nuspec.xsd\">
          <metadata>
            <id>zagel</id>
            <version>{version}</version>
            <title>Zagel</title>
            <authors>sharno</authors>
            <projectUrl>{metadata["repository"]}</projectUrl>
            <license type=\"expression\">{metadata["license"]}</license>
            <requireLicenseAcceptance>false</requireLicenseAcceptance>
            <description>{metadata["description"]}</description>
            <tags>zagel rest http client gui</tags>
          </metadata>
        </package>
        """
    )


def build_choco_install_script(windows_asset: ReleaseAsset) -> str:
    return textwrap.dedent(
        f"""\
        $ErrorActionPreference = 'Stop'
        $packageName = 'zagel'
        $toolsDir = Split-Path -Parent $MyInvocation.MyCommand.Definition

        $packageArgs = @{{
          packageName   = $packageName
          unzipLocation = $toolsDir
          url64bit      = '{windows_asset.url}'
          checksum64    = '{windows_asset.sha256}'
          checksumType64 = 'sha256'
        }}

        Install-ChocolateyZipPackage @packageArgs
        Install-BinFile -Name 'zagel' -Path (Join-Path $toolsDir 'zagel.exe')
        """
    )


def build_choco_uninstall_script() -> str:
    return "Uninstall-BinFile -Name 'zagel'\n"


def build_aur_pkgbuild(
    version: str, metadata: dict[str, str], linux_asset: ReleaseAsset
) -> str:
    return textwrap.dedent(
        f"""\
        pkgname=zagel-bin
        pkgver={version}
        pkgrel=1
        pkgdesc='{metadata["description"]}'
        arch=('x86_64')
        url='{metadata["repository"]}'
        license=('{metadata["license"]}')
        depends=('glibc')
        source=('zagel-v${{pkgver}}-x86_64-unknown-linux-gnu.tar.gz::{linux_asset.url}')
        sha256sums=('{linux_asset.sha256}')

        package() {{
          install -Dm755 "${{srcdir}}/zagel" "${{pkgdir}}/usr/bin/zagel"
        }}
        """
    )


def build_packaging_readme() -> str:
    return textwrap.dedent(
        """\
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
        """
    )


def main() -> int:
    args = parse_args()
    metadata = read_cargo_metadata()
    tag, assets = load_assets(args.release_json)
    version = tag[1:] if tag.startswith("v") else tag

    linux_asset = find_asset(assets, "x86_64-unknown-linux-gnu.tar.gz")
    windows_asset = find_asset(assets, "x86_64-pc-windows-msvc.zip")
    mac_x64_asset = find_asset(assets, "x86_64-apple-darwin.tar.gz")
    mac_arm_asset = find_asset(assets, "aarch64-apple-darwin.tar.gz")

    out_dir = args.output_dir
    winget_base = out_dir / "winget" / "manifests" / "s" / "Sharno" / "Zagel" / version

    write_text(out_dir / "README.md", build_packaging_readme())
    write_text(
        out_dir / "homebrew" / "zagel.rb",
        build_homebrew_formula(
            version, metadata, linux_asset, mac_x64_asset, mac_arm_asset
        ),
    )
    write_text(
        out_dir / "scoop" / "zagel.json",
        build_scoop_manifest(version, metadata, windows_asset),
    )
    write_text(
        winget_base / "Sharno.Zagel.yaml",
        build_winget_version_manifest(version),
    )
    write_text(
        winget_base / "Sharno.Zagel.installer.yaml",
        build_winget_installer_manifest(version, windows_asset),
    )
    write_text(
        winget_base / "Sharno.Zagel.locale.en-US.yaml",
        build_winget_locale_manifest(version, metadata),
    )
    write_text(
        out_dir / "chocolatey" / "zagel.nuspec",
        build_choco_nuspec(version, metadata),
    )
    write_text(
        out_dir / "chocolatey" / "tools" / "chocolateyinstall.ps1",
        build_choco_install_script(windows_asset),
    )
    write_text(
        out_dir / "chocolatey" / "tools" / "chocolateyuninstall.ps1",
        build_choco_uninstall_script(),
    )
    write_text(
        out_dir / "aur" / "PKGBUILD",
        build_aur_pkgbuild(version, metadata, linux_asset),
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
