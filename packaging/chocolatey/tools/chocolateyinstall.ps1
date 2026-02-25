$ErrorActionPreference = 'Stop'
$packageName = 'zagel'
$toolsDir = Split-Path -Parent $MyInvocation.MyCommand.Definition

$packageArgs = @{
  packageName   = $packageName
  unzipLocation = $toolsDir
  url64bit      = 'https://github.com/sharno/zagel/releases/download/v0.3.0/zagel-v0.3.0-x86_64-pc-windows-msvc.zip'
  checksum64    = 'df65dd7fa19335b681ea9705a6acad3b38099c07c912164450c7ab0489039107'
  checksumType64 = 'sha256'
}

Install-ChocolateyZipPackage @packageArgs
Install-BinFile -Name 'zagel' -Path (Join-Path $toolsDir 'zagel.exe')
