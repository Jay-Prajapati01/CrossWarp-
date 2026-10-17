[CmdletBinding()]
param(
    [ValidateSet('Debug', 'Release')]
    [string] $Configuration = 'Debug'
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$installerRoot = 'C:\Program Files (x86)\Microsoft Visual Studio\Installer'
$vswhere = Join-Path $installerRoot 'vswhere.exe'
if (-not (Test-Path $vswhere)) { throw "vswhere.exe not found: $vswhere" }

$visualStudioPath = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $visualStudioPath) { throw 'No VS 2022 x64 MSVC toolchain was found.' }
$devCmd = Join-Path $visualStudioPath 'Common7\Tools\VsDevCmd.bat'
$baseTargets = Join-Path $visualStudioPath 'MSBuild\Microsoft\VC\v170'
if (-not (Test-Path $devCmd) -or -not (Test-Path $baseTargets)) { throw 'The selected VS instance does not contain the VS 2022 C++ MSBuild targets.' }

# VS 17.14 / WDK 10.0.26100.6584 catalog payload. Its hash is pinned from the local Microsoft VS catalog.
$vsixUrl = 'https://download.visualstudio.microsoft.com/download/pr/fa1259b6-3659-4a26-a8b4-c42d40b343ab/c53677dd5d56679c4298323fd12a1ec504cc05e9e346c0866b178955a9dcbf4b/payload.vsix'
$expectedHash = 'C53677DD5D56679C4298323FD12A1EC504CC05E9E346C0866B178955A9DCBF4B'
$tempRoot = Join-Path $env:TEMP ('unified-wdk-vctargets-' + [guid]::NewGuid().ToString('N'))
$mergedTargets = Join-Path $tempRoot 'v170'
$unpackRoot = Join-Path $tempRoot 'unpacked'
$vsixPath = Join-Path $tempRoot 'wdk-build.vsix'
New-Item -ItemType Directory -Path $mergedTargets, $unpackRoot | Out-Null
Copy-Item (Join-Path $baseTargets '*') $mergedTargets -Recurse -Force
Invoke-WebRequest -Uri $vsixUrl -OutFile $vsixPath
$actualHash = (Get-FileHash $vsixPath -Algorithm SHA256).Hash
if ($actualHash -ne $expectedHash) { throw "WDK VSIX hash mismatch: $actualHash" }

Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::ExtractToDirectory($vsixPath, $unpackRoot)
$wdkTargets = Join-Path $unpackRoot 'Contents\MSBuild\Microsoft\VC\v170'
if (-not (Test-Path (Join-Path $wdkTargets 'Platforms\x64\PlatformToolsets\WindowsUserModeDriver10.0\Toolset.props'))) {
    throw 'Pinned payload is missing the x64 WDK MSBuild platform toolset.'
}
Copy-Item (Join-Path $wdkTargets '*') $mergedTargets -Recurse -Force

$sample = Join-Path $repoRoot 'spikes\phase0\microsoft-idd-sample\IddSampleDriver.sln'
if (-not (Test-Path $sample)) { throw "Microsoft IDD sample solution not found: $sample" }
$quotedDevCmd = '"' + $devCmd + '"'
$arguments = @(
    "call $quotedDevCmd -arch=x64 -host_arch=x64 &&",
    'msbuild', ('"' + $sample + '"'), '/m', "/p:Configuration=$Configuration", '/p:Platform=x64',
    '/p:SignMode=Off', '/p:Inf2CatUseLocalTime=true', '/p:SpectreMitigation=false',
    '/p:WindowsTargetPlatformVersion=10.0.26100.0', ('/p:VCTargetsPath=' + $mergedTargets + '\'), '/v:minimal'
) -join ' '

Push-Location $repoRoot
try {
    & cmd.exe /d /c $arguments
    if ($LASTEXITCODE -ne 0) { throw "MSBuild exited with code $LASTEXITCODE" }
} finally {
    Pop-Location
}

Write-Host 'IDD sample compile/link and unsigned catalog generation passed. Driver signing and installation are intentionally not performed.'
Write-Host "Temporary WDK targets overlay: $tempRoot"
