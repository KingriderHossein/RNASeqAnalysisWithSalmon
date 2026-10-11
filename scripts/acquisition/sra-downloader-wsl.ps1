#requires -Version 5.1
<#
.SYNOPSIS
  SRA Sequential Downloader v1.5.0 - Windows + WSL2 launcher.
.DESCRIPTION
  Invokes the adjacent Linux downloader using wsl.exe. SRA is downloaded by
  NCBI prefetch inside Ubuntu; this file does not implement network transfer.
  No Windows settings, user data or WSL configuration is changed.
.EXAMPLE
  .\sra-downloader-wsl.ps1 -Check -Distribution Ubuntu
.EXAMPLE
  .\sra-downloader-wsl.ps1 -Run SRR4453804 -DryRun -Distribution Ubuntu
.EXAMPLE
  .\sra-downloader-wsl.ps1 -InputFile '.\raw\acquisition\GSE89223_runs.txt' -OutputDir 'D:\RNA Data' -Distribution Ubuntu
#>
[CmdletBinding()]
param(
    [Alias('r')]
    [string[]] $Run = @(),
    [Alias('i')]
    [string] $InputFile,
    [Alias('o')]
    [string] $OutputDir,
    [Alias('d')]
    [string] $Distribution,
    [ValidateRange(1, 1000000)]
    [int] $MaxAttempts = 10,
    [ValidateRange(0, 1000000)]
    [int] $MinFreeGiB = 20,
    [ValidateSet('checksum', 'full')]
    [string] $VerifyExisting = 'checksum',
    [ValidatePattern('^(u|[0-9]{1,15}[kKmMgGtT]?)$')]
    [string] $MaxSize = 'u',
    [ValidateRange(0, 1000000)]
    [int] $BaseBackoffSeconds = 15,
    [ValidateRange(0, 1000000)]
    [int] $MaxBackoffSeconds = 300,
    [ValidateRange(0, 1000000)]
    [int] $StallTimeoutSeconds = 0,
    [ValidateRange(1, 1000000)]
    [int] $StallPollSeconds = 30,
    [string] $RequireMountpoint,
    [switch] $ContinueOnFailure,
    [switch] $DryRun,
    [switch] $Check,
    [switch] $Help,
    [switch] $Version
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Show-Help {
    @'
SRA Sequential Downloader v1.5.0 - Windows + WSL2

USAGE (run from PowerShell with the repository cloned to C: or D:):
  .\scripts\acquisition\sra-downloader-wsl.ps1 -Check -Distribution Ubuntu
  .\scripts\acquisition\sra-downloader-wsl.ps1 -Run SRR4453804 -DryRun
  .\scripts\acquisition\sra-downloader-wsl.ps1 -InputFile .\raw\acquisition\GSE89223_runs.txt
  .\scripts\acquisition\sra-downloader-wsl.ps1 -Run SRR4453804 -OutputDir 'D:\RNA Data'

OPTIONS
  -Run                One or more SRR/ERR/DRR accessions
  -InputFile          Existing Windows file path or absolute Linux WSL path
  -OutputDir          Windows drive path or absolute Linux path; default: Linux $HOME/sra_data
  -Distribution       Installed WSL distribution; omit for the default distro
  -Check              Inspect WSL prerequisites; no download
  -DryRun             Preview exact downloader plan; no output created
  -RequireMountpoint  Windows drive root (D:\) or Linux mount path
  -MinFreeGiB         Reserve GiB before/during transfer (default 20)
  -MaxAttempts        Attempts per Run (default 10)
  -MaxSize            prefetch limit (default u = unlimited)
  -VerifyExisting     checksum (default) or full
  -ContinueOnFailure  Continue to next Run after exhausting retries
  -BaseBackoffSeconds, -MaxBackoffSeconds, -StallTimeoutSeconds, -StallPollSeconds
  -Help, -Version

NOTES
  The Bash file sra_downloader_v1.5.0.sh must be next to this launcher.
  Output is an SRA archive, NOT FASTQ. All transfers occur inside WSL.
  No WSL installation or Windows execution-policy change is performed.
'@ | Write-Output
}

function Get-WslPrefix {
    if (-not [string]::IsNullOrWhiteSpace($Distribution)) {
        Write-Output '-d'
        Write-Output $Distribution
    }
}

function Invoke-WslText {
    param([Parameter(Mandatory = $true)][string[]] $ExecArguments)
    $invokeArgs = @(Get-WslPrefix) + @('--exec') + $ExecArguments
    $response = & wsl.exe @invokeArgs 2>&1
    $code = $LASTEXITCODE
    if ($code -ne 0) {
        throw ('WSL command exited {0}: {1}. Verify WSL2 and -Distribution.' -f $code, (($response | Out-String).Trim()))
    }
    return (($response | Out-String).Trim())
}

function Resolve-WslPath {
    param(
        [Parameter(Mandatory = $true)][string] $Value,
        [switch] $ExistingWindowsFile
    )
    if ([string]::IsNullOrWhiteSpace($Value)) { throw 'Empty path is not valid.' }
    if ($Value.StartsWith('/')) { return $Value }
    if ($Value -eq '~' -or $Value.StartsWith('~/')) {
        $homeDir = Invoke-WslText -ExecArguments @('printenv', 'HOME')
        if ([string]::IsNullOrWhiteSpace($homeDir) -or -not $homeDir.StartsWith('/')) { throw 'Invalid WSL HOME.' }
        if ($Value -eq '~') { return $homeDir }
        return ($homeDir.TrimEnd('/') + '/' + $Value.Substring(2))
    }
    if ($Value.StartsWith('\\')) { throw 'UNC paths are unsupported. Use C:\..., D:\..., or /home/...' }
    if ($Value -match '^[A-Za-z]:$') { throw 'A drive letter without a path is unsafe; use D:\folder.' }
    if ($ExistingWindowsFile) {
        if (-not (Test-Path -LiteralPath $Value -PathType Leaf)) {
            throw ('Input file not found: {0}' -f $Value)
        }
        $winPath = (Get-Item -LiteralPath $Value).FullName
    } else {
        if ([System.IO.Path]::IsPathRooted($Value)) {
            $winPath = [System.IO.Path]::GetFullPath($Value)
        } else {
            $winPath = [System.IO.Path]::GetFullPath((Join-Path -Path (Get-Location).ProviderPath -ChildPath $Value))
        }
    }
    if ($winPath -notmatch '^[A-Za-z]:\\') {
        throw ('Unsupported Windows path: {0}; use a drive-letter path or Linux /home/...' -f $winPath)
    }
    $linuxPath = Invoke-WslText -ExecArguments @('wslpath', '-a', '-u', $winPath)
    if ([string]::IsNullOrWhiteSpace($linuxPath) -or -not $linuxPath.StartsWith('/')) {
        throw ('Could not convert path to WSL: {0}' -f $winPath)
    }
    return $linuxPath
}

if ($Help) { Show-Help; exit 0 }
if ($Version) { Write-Output '1.5.0'; exit 0 }

try {
    if (-not (Get-Command 'wsl.exe' -ErrorAction SilentlyContinue)) {
        throw 'wsl.exe is unavailable. Install and configure WSL2/Ubuntu first.'
    }
    $kernel = Invoke-WslText -ExecArguments @('cat', '/proc/sys/kernel/osrelease')
    if ($kernel -notmatch '(?i)wsl2') {
        Write-Warning 'The Linux kernel does not advertise WSL2; inspect wsl --list --verbose.'
    }
    if ($Check) {
        $bashVersion = Invoke-WslText -ExecArguments @('bash', '--version')
        Write-Host ('WSL kernel: ' + $kernel)
        Write-Host ('Bash: ' + (($bashVersion -split [Environment]::NewLine)[0]))
        Write-Host ('prefetch: ' + (Invoke-WslText -ExecArguments @('prefetch', '--version')))
        Write-Host ('vdb-validate: ' + (Invoke-WslText -ExecArguments @('vdb-validate', '--version')))
        Write-Host 'Prerequisite commands are available. No data was downloaded.'
        exit 0
    }
    if (($Run.Count -eq 0) -and [string]::IsNullOrWhiteSpace($InputFile)) {
        throw 'Specify -Run SRR... or -InputFile path; use -Help.'
    }
    $scriptWindowsPath = Join-Path -Path $PSScriptRoot -ChildPath 'sra_downloader_v1.5.0.sh'
    if (-not (Test-Path -LiteralPath $scriptWindowsPath -PathType Leaf)) {
        throw ('Required Bash file is missing: {0}' -f $scriptWindowsPath)
    }
    $scriptLinuxPath = Resolve-WslPath -Value $scriptWindowsPath -ExistingWindowsFile

    $autoMountLinux = ''
    if ([string]::IsNullOrWhiteSpace($OutputDir)) {
        $homeDir = Invoke-WslText -ExecArguments @('printenv', 'HOME')
        if (-not $homeDir.StartsWith('/')) { throw 'Linux HOME must be an absolute path.' }
        $linuxOutput = $homeDir.TrimEnd('/') + '/sra_data'
    } else {
        $linuxOutput = Resolve-WslPath -Value $OutputDir
        if ($OutputDir -match '^([A-Za-z]):[\\/]') {
            $driveRoot = ('{0}:\' -f $Matches[1])
            $autoMountLinux = (Resolve-WslPath -Value $driveRoot).TrimEnd('/')
            if ($linuxOutput.TrimEnd('/') -eq $autoMountLinux) {
                throw 'Refusing to use the Windows drive root; specify a subfolder.'
            }
            Write-Warning ('Windows-mounted storage may be slower than WSL2 ext4: {0}' -f $linuxOutput)
        }
    }
    $linuxMount = ''
    if (-not [string]::IsNullOrWhiteSpace($RequireMountpoint)) {
        $linuxMount = Resolve-WslPath -Value $RequireMountpoint
    } elseif (-not [string]::IsNullOrWhiteSpace($autoMountLinux)) {
        $linuxMount = $autoMountLinux
        Write-Host ('Automatic mountpoint guard: ' + $linuxMount)
    }

    $linuxArgs = @()
    foreach ($accession in $Run) { $linuxArgs += @('--run', $accession) }
    if (-not [string]::IsNullOrWhiteSpace($InputFile)) {
        $isWindowsFile = (-not $InputFile.StartsWith('/')) -and (-not $InputFile.StartsWith('~/')) -and ($InputFile -ne '~')
        $linuxInput = Resolve-WslPath -Value $InputFile -ExistingWindowsFile:$isWindowsFile
        $linuxArgs += @('--input', $linuxInput)
    }
    $linuxArgs += @('--output', $linuxOutput)
    $linuxArgs += @('--max-attempts', [string]$MaxAttempts)
    $linuxArgs += @('--min-free-gb', [string]$MinFreeGiB)
    $linuxArgs += @('--max-size', $MaxSize)
    $linuxArgs += @('--verify-existing', $VerifyExisting)
    $linuxArgs += @('--base-backoff', [string]$BaseBackoffSeconds)
    $linuxArgs += @('--max-backoff', [string]$MaxBackoffSeconds)
    $linuxArgs += @('--stall-timeout', [string]$StallTimeoutSeconds)
    $linuxArgs += @('--stall-poll', [string]$StallPollSeconds)
    if ($ContinueOnFailure) { $linuxArgs += '--continue-on-failure' }
    if ($DryRun) { $linuxArgs += '--dry-run' }
    if (-not [string]::IsNullOrWhiteSpace($linuxMount)) {
        $linuxArgs += @('--require-mountpoint', $linuxMount)
    }

    Write-Host ('WSL output directory: ' + $linuxOutput)
    $launchArgs = @(Get-WslPrefix) + @('--exec', 'bash', $scriptLinuxPath) + $linuxArgs
    & wsl.exe @launchArgs
    exit $LASTEXITCODE
} catch {
    [Console]::Error.WriteLine(('ERROR: ' + $_.Exception.Message))
    exit 1
}
