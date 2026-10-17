[CmdletBinding()]
param(
    [ValidateSet('Debug', 'Release')]
    [string] $Configuration = 'Debug'
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$vswhere = 'C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path $vswhere)) { throw "vswhere.exe not found: $vswhere" }

# The WDK VSIX must be installed in VS. Use 64-bit MSBuild: the x86 host on
# this fixture tries to load a missing x86 InfVerif.dll before Inf2Cat runs.
$visualStudioPath = & $vswhere -latest -products '*' `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 Component.Microsoft.Windows.DriverKit `
    -property installationPath
if (-not $visualStudioPath) {
    throw 'Visual Studio 2022 with the Windows Driver Kit component was not found. Add Windows Driver Kit in Visual Studio Installer > Modify > Individual Components.'
}
$devCmd = Join-Path $visualStudioPath 'Common7\Tools\VsDevCmd.bat'
$msbuild = Join-Path $visualStudioPath 'MSBuild\Current\Bin\amd64\MSBuild.exe'
if (-not (Test-Path $devCmd) -or -not (Test-Path $msbuild)) {
    throw 'The selected Visual Studio installation is missing VsDevCmd.bat or 64-bit MSBuild.'
}

$sample = Join-Path $repoRoot 'spikes\phase0\microsoft-idd-sample\IddSampleDriver.sln'
if (-not (Test-Path $sample)) { throw "Microsoft IDD sample solution not found: $sample" }
$command = @(
    'call "' + $devCmd + '" -arch=x64 -host_arch=x64 &&',
    '"' + $msbuild + '"', ('"' + $sample + '"'), '/m',
    "/p:Configuration=$Configuration", '/p:Platform=x64', '/p:SignMode=Off',
    '/p:Inf2CatUseLocalTime=true', '/p:SpectreMitigation=false',
    '/p:WindowsTargetPlatformVersion=10.0.26100.0', '/v:minimal'
) -join ' '

Push-Location $repoRoot
try {
    & cmd.exe /d /c $command
    if ($LASTEXITCODE -ne 0) { throw "MSBuild exited with code $LASTEXITCODE" }
} finally {
    Pop-Location
}

Write-Host 'System-integrated IDD sample compile/link and unsigned catalog generation passed. Driver signing and installation are intentionally not performed.'
