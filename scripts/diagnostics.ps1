[CmdletBinding()]
param()

$ErrorActionPreference = 'SilentlyContinue'

function Get-CommandPath([string] $Name) {
    $command = Get-Command $Name -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $command -and $Name -in @('rustup', 'rustc', 'cargo')) {
        $rustPath = Join-Path $env:USERPROFILE ".cargo\bin\$Name.exe"
        if (Test-Path $rustPath) { return $rustPath }
    }
    if ($null -eq $command) { return $null }
    return $command.Source
}

$os = Get-CimInstance Win32_OperatingSystem
$computer = Get-CimInstance Win32_ComputerSystem
$principal = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
$secureBoot = try { Confirm-SecureBootUEFI -ErrorAction Stop } catch { "unavailable: $($_.Exception.Message)" }
$testSigning = (& bcdedit.exe /enum '{current}' 2>$null | Select-String 'testsigning').Line
$monitors = @(Get-CimInstance Win32_DesktopMonitor | ForEach-Object {
    [ordered]@{ name = $_.Name; deviceId = $_.PNPDeviceID }
})
$graphics = @(Get-CimInstance Win32_VideoController | ForEach-Object {
    [ordered]@{
        name = $_.Name
        driverVersion = $_.DriverVersion
        resolution = if ($_.CurrentHorizontalResolution -and $_.CurrentVerticalResolution) {
            "$($_.CurrentHorizontalResolution)x$($_.CurrentVerticalResolution)"
        } else { $null }
    }
})

$toolNames = @('git', 'node', 'npm', 'cmake', 'msbuild', 'cl', 'link', 'rustup', 'rustc', 'cargo', 'inf2cat', 'signtool', 'pnputil')
$tools = [ordered]@{}
foreach ($name in $toolNames) { $tools[$name] = Get-CommandPath $name }

$network = [ordered]@{
    adapters = @(Get-NetAdapter -Physical | ForEach-Object {
        [ordered]@{
            name = $_.Name
            status = $_.Status.ToString()
            linkSpeed = $_.LinkSpeed.ToString()
            description = $_.InterfaceDescription
        }
    })
    ipv4 = @(Get-NetIPAddress -AddressFamily IPv4 | Where-Object {
        $_.IPAddress -notlike '169.254*' -and $_.IPAddress -ne '127.0.0.1'
    } | ForEach-Object {
        [ordered]@{ interface = $_.InterfaceAlias; address = $_.IPAddress; prefixLength = $_.PrefixLength }
    })
}

$vswhere = 'C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe'
$visualStudio = @()
if (Test-Path $vswhere) {
    $instances = & $vswhere -all -products '*' -format json 2>$null | ConvertFrom-Json
    $visualStudio = @($instances | ForEach-Object {
        [ordered]@{ installationPath = $_.installationPath; displayName = $_.displayName; installationVersion = $_.installationVersion }
    })
}

if ($visualStudio.Count -gt 0) {
    $vsRoot = $visualStudio[0].installationPath
    if (-not $tools.msbuild) {
        $candidate = Join-Path $vsRoot 'MSBuild\Current\Bin\MSBuild.exe'
        if (Test-Path $candidate) { $tools.msbuild = $candidate }
    }
    $msvcRoot = Join-Path $vsRoot 'VC\Tools\MSVC'
    $msvcVersion = Get-ChildItem $msvcRoot -Directory | Sort-Object Name -Descending | Select-Object -First 1
    if ($null -ne $msvcVersion) {
        foreach ($tool in @('cl', 'link')) {
            if (-not $tools[$tool]) {
                $candidate = Join-Path $msvcVersion.FullName "bin\HostX64\x64\$tool.exe"
                if (Test-Path $candidate) { $tools[$tool] = $candidate }
            }
        }
    }
}

$windowsKits = @()
foreach ($root in @('C:\Program Files (x86)\Windows Kits\10', 'C:\Program Files\Windows Kits\10')) {
    if (Test-Path $root) {
        $windowsKits += [ordered]@{
            path = $root
            includeVersions = @(Get-ChildItem (Join-Path $root 'Include') -Directory | Select-Object -ExpandProperty Name)
            buildVersions = @(Get-ChildItem (Join-Path $root 'Build') -Directory | Select-Object -ExpandProperty Name)
        }
    }
}

$kitRootPath = 'C:\Program Files (x86)\Windows Kits\10'
$kitBin = Join-Path $kitRootPath 'bin'
$kitToolVersions = Get-ChildItem $kitBin -Directory | Sort-Object Name -Descending
if (-not $tools.signtool -or $tools.signtool -match '\\arm\\') {
    $candidate = $kitToolVersions | ForEach-Object {
        $path = Join-Path $_.FullName 'x64\signtool.exe'
        if (Test-Path $path) { Get-Item $path }
    } | Select-Object -First 1
    if ($candidate) { $tools.signtool = $candidate.FullName }
}
if (-not $tools.inf2cat) {
    $candidate = $kitToolVersions | ForEach-Object {
        $path = Join-Path $_.FullName 'x86\Inf2Cat.exe'
        if (Test-Path $path) { Get-Item $path }
    } | Select-Object -First 1
    if ($candidate) { $tools.inf2cat = $candidate.FullName }
}

[ordered]@{
    capturedAtUtc = [DateTime]::UtcNow.ToString('o')
    computerName = $env:COMPUTERNAME
    manufacturer = $computer.Manufacturer
    model = $computer.Model
    operatingSystem = [ordered]@{
        caption = $os.Caption
        version = $os.Version
        build = $os.BuildNumber
        architecture = $os.OSArchitecture
    }
    graphics = $graphics
    monitors = $monitors
    elevatedAdministrator = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    secureBoot = $secureBoot
    testSigningEntry = $testSigning
    tools = $tools
    visualStudioInstances = $visualStudio
    windowsKits = $windowsKits
    network = $network
} | ConvertTo-Json -Depth 8

