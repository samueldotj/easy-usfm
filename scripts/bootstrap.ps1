<#
.SYNOPSIS
    Install the toolchain easy-usfm needs, on Windows.

.DESCRIPTION
    Idempotent: anything already present is left alone. Uses winget, which ships
    with Windows 10 1809+ and Windows 11.

    Default installs everything the project needs. Use -Minimal for just the
    corpus tooling (Python only), which is all that is required today.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\bootstrap.ps1
.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\bootstrap.ps1 -Minimal
#>

#Requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$Minimal,          # Python only — enough for the corpus tooling
    [switch]$SkipRust,
    [switch]$SkipNode,
    [switch]$SkipJust,
    [switch]$SkipBuildTools    # use the gnu target rather than installing MSVC
)

$ErrorActionPreference = 'Stop'

# --------------------------------------------------------------------------

function Write-Step ($m) { Write-Host "`n=> $m" -ForegroundColor Cyan }
function Write-Ok   ($m) { Write-Host "   ok    $m" -ForegroundColor Green }
function Write-Skip ($m) { Write-Host "   have  $m" -ForegroundColor DarkGray }
function Write-Warn ($m) { Write-Host "   warn  $m" -ForegroundColor Yellow }
function Write-Err  ($m) { Write-Host "   FAIL  $m" -ForegroundColor Red }

# winget updates the registry, not this process. Without re-reading PATH the
# verification below would fail for something we just installed successfully.
function Update-SessionPath {
    $machine = [Environment]::GetEnvironmentVariable('Path', 'Machine')
    $user    = [Environment]::GetEnvironmentVariable('Path', 'User')
    $env:Path = ($machine, $user | Where-Object { $_ }) -join ';'
}

function Test-Tool ($name) {
    return [bool](Get-Command $name -ErrorAction SilentlyContinue)
}

# Windows PowerShell 5.1 turns a native command's stderr into ErrorRecords when
# it is redirected, and $ErrorActionPreference = 'Stop' then makes those
# terminating. rustup and cargo write ordinary progress to stderr, so capturing
# their output naively aborts the script on success. Returns combined output;
# the caller checks $LASTEXITCODE.
function Invoke-Native {
    param([Parameter(Mandatory)][string]$Exe, [string[]]$Arguments = @())
    $prev = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try { return (& $Exe @Arguments 2>&1 | Out-String) }
    finally { $ErrorActionPreference = $prev }
}

# The Microsoft Store puts alias stubs on PATH at WindowsApps\python3.exe. They
# are real executables that Get-Command finds, but they only print an
# advertisement and exit non-zero. The interpreter has to be run to know
# whether it is actually installed.
function Test-Python ($cmd) {
    if (-not (Test-Tool $cmd)) { return $false }
    $null = Invoke-Native $cmd @('--version')
    return ($LASTEXITCODE -eq 0)
}

function Install-Pkg ($id, $label, $probe) {
    if (Test-Tool $probe) { Write-Skip "$label"; return $true }

    Write-Host "   installing $label ($id)…"
    # Exit codes vary between "installed", "already installed", and "no upgrade
    # applicable", so trust the probe rather than the code.
    Invoke-Native winget @(
        'install', '--id', $id, '--source', 'winget', '--exact', '--silent',
        '--accept-package-agreements', '--accept-source-agreements') | Write-Verbose

    Update-SessionPath
    if (Test-Tool $probe) { Write-Ok "$label"; return $true }

    Write-Warn "$label did not appear on PATH — a new terminal may be needed"
    return $false
}

# --------------------------------------------------------------------------

Write-Host "easy-usfm bootstrap" -ForegroundColor White
Write-Host "-------------------"

Write-Step "Checking winget"
if (-not (Test-Tool 'winget')) {
    Write-Err "winget not found."
    Write-Host @"

   winget ships with Windows 11 and Windows 10 1809+. If it is missing,
   install 'App Installer' from the Microsoft Store:

       https://apps.microsoft.com/detail/9nblggh4nns1

   Then run this script again. To install Python by hand instead:

       https://www.python.org/downloads/windows/
       (tick 'Add python.exe to PATH' in the installer)
"@
    exit 1
}
Write-Ok "winget present"

# ---- Python — required -----------------------------------------------------

Write-Step "Python 3 (required by the corpus tooling)"
$havePython = (Test-Python 'py') -or (Test-Python 'python3') -or (Test-Python 'python')
if ($havePython) {
    Write-Skip "python already installed"
} else {
    Install-Pkg 'Python.Python.3.12' 'Python 3.12' 'py' | Out-Null
}

# ---- Optional toolchain ----------------------------------------------------

if (-not $Minimal) {

    if (-not $SkipJust) {
        Write-Step "just (task runner — optional)"
        if (-not (Install-Pkg 'Casey.Just' 'just' 'just')) {
            Write-Warn "falling back to cargo later if Rust is installed"
        }
    }

    if (-not $SkipRust) {
        Write-Step "Rust toolchain"

        # rustup defaults to the msvc target, which links with link.exe from
        # the Visual C++ build tools. Without them nothing compiles at all --
        # not even a wasm-only build, because proc-macro crates are built for
        # the host. Tauri targets msvc on Windows too, so this is the right
        # toolchain rather than a workaround; -SkipBuildTools switches to the
        # gnu target instead, which brings its own linker.
        if ($SkipBuildTools) {
            Write-Warn "skipping Visual C++ build tools; using the gnu target"
            if (Test-Tool 'rustup') {
                Invoke-Native rustup @('default', 'stable-x86_64-pc-windows-gnu') |
                    Write-Verbose
            }
        } elseif (Test-Path "$env:ProgramFiles(x86)\Microsoft Visual Studio\Installer\vswhere.exe") {
            Write-Skip "Visual C++ build tools"
        } else {
            Write-Host "   installing Visual C++ build tools (several GB)…"
            Invoke-Native winget @(
                'install', '--id', 'Microsoft.VisualStudio.2022.BuildTools',
                '--source', 'winget', '--exact', '--silent',
                '--accept-package-agreements', '--accept-source-agreements',
                '--override',
                '--wait --quiet --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
            ) | Write-Verbose
            Write-Ok "Visual C++ build tools"
        }

        if (Install-Pkg 'Rustlang.Rustup' 'rustup' 'rustup') {
            Update-SessionPath
            if (Test-Tool 'rustup') {
                Write-Host "   adding wasm32-unknown-unknown target…"
                Invoke-Native rustup @('target', 'add', 'wasm32-unknown-unknown') |
                    Write-Verbose
                Write-Ok "wasm32-unknown-unknown"
            }
            # just is a Rust program; cargo is a reliable fallback if winget
            # did not have it.
            if (-not $SkipJust -and -not (Test-Tool 'just') -and (Test-Tool 'cargo')) {
                Write-Host "   installing just via cargo…"
                Invoke-Native cargo @('install', 'just') | Write-Verbose
                Update-SessionPath
                if (Test-Tool 'just') { Write-Ok "just (via cargo)" }
            }
        }
    }

    if (-not $SkipNode) {
        Write-Step "Node.js LTS (frontend, and the usfm-grammar test oracle)"
        Install-Pkg 'OpenJS.NodeJS.LTS' 'Node.js LTS' 'node' | Out-Null
    }
}

# ---- Verify ----------------------------------------------------------------

Update-SessionPath
Write-Step "Verifying"

$python = $null
foreach ($c in @('py -3', 'python3', 'python')) {
    $parts = $c.Split(' ')
    $exe   = $parts[0]
    if (Test-Tool $exe) {
        $v = Invoke-Native $exe (@($parts | Select-Object -Skip 1) + '--version')
        if ($LASTEXITCODE -eq 0) { $python = $c; Write-Ok "python  $c  ($($v.Trim()))"; break }
    }
}
if (-not $python) { Write-Err "no working python found" }

foreach ($t in @('just', 'cargo', 'node', 'git')) {
    if (Test-Tool $t) {
        $v = (Invoke-Native $t @('--version')).Trim() -split "`r?`n" | Select-Object -First 1
        Write-Ok ("$t".PadRight(7) + " $v")
    } elseif ($Minimal -and $t -ne 'git') {
        Write-Skip "$t (skipped: -Minimal)"
    } else {
        Write-Warn "$t not found"
    }
}

# ---- Smoke test ------------------------------------------------------------

if ($python) {
    Write-Step "Smoke test — corpus tooling self-test"
    $repo = Split-Path $PSScriptRoot -Parent
    Push-Location $repo
    try {
        & cmd /c "$python tools\corpus\test_tooling.py"
        if ($LASTEXITCODE -eq 0) { Write-Ok "corpus tooling works" }
        else { Write-Err "self-test failed (exit $LASTEXITCODE)" }
    } finally { Pop-Location }
}

# ---- Next ------------------------------------------------------------------

Write-Host "`nNext" -ForegroundColor White
Write-Host @"

   If anything above says 'not found', open a NEW terminal first — PATH
   changes do not reach a shell that was already running.

   Build the test corpus:

       $python tools\corpus\fetch.py --dry-run     # see what would download
       $python tools\corpus\fetch.py               # download (~10 min)
       $python tools\corpus\select.py corpus\extended --target 200 --copy-to corpus\core
       $python tools\corpus\verify.py

   Or, with just installed:

       just corpus-rebuild

   See corpus\README.md for what the tiers are and why.
"@
