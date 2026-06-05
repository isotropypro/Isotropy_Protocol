param(
    [string]$ArtifactsDir = ".\artifacts",
    [string]$TargetDir = "target_compat_wasm",
    [string]$RustToolchain = "stable",
    [string]$WasmOptBin = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function New-GzipArtifact {
    param(
        [string]$SourcePath,
        [string]$DestinationPath
    )

    $sourceBytes = [System.IO.File]::ReadAllBytes($SourcePath)
    $destinationStream = [System.IO.File]::Create($DestinationPath)
    try {
        $gzipStream = New-Object System.IO.Compression.GzipStream(
            $destinationStream,
            [System.IO.Compression.CompressionLevel]::Optimal
        )
        try {
            $gzipStream.Write($sourceBytes, 0, $sourceBytes.Length)
        }
        finally {
            $gzipStream.Dispose()
        }
    }
    finally {
        $destinationStream.Dispose()
    }
}

function Resolve-WasmOpt {
    if (-not [string]::IsNullOrWhiteSpace($WasmOptBin)) {
        if (-not (Test-Path -LiteralPath $WasmOptBin)) {
            throw "Specified wasm-opt binary was not found: $WasmOptBin"
        }
        return (Resolve-Path -LiteralPath $WasmOptBin).Path
    }

    $command = Get-Command "wasm-opt" -ErrorAction SilentlyContinue
    if ($null -ne $command) {
        return $command.Source
    }

    return $null
}

function Invoke-WasmOpt {
    param(
        [string]$BinaryPath,
        [string]$WasmPath
    )

    $optimizedPath = "$WasmPath.opt"
    Write-Host ""
    Write-Host "> $BinaryPath -Oz --signext-lowering $WasmPath -o $optimizedPath" -ForegroundColor Cyan
    & $BinaryPath '-Oz' '--signext-lowering' $WasmPath '-o' $optimizedPath
    if ($LASTEXITCODE -ne 0) {
        throw "wasm-opt failed with exit code $LASTEXITCODE."
    }
    Move-Item -LiteralPath $optimizedPath -Destination $WasmPath -Force
}

function Assert-CommandAvailable {
    param([string]$CommandName)

    if (-not (Get-Command $CommandName -ErrorAction SilentlyContinue)) {
        throw "Required command '$CommandName' was not found in PATH."
    }
}

function Invoke-Build {
    param([string[]]$CargoArgs)

    Write-Host ""
    Write-Host "> rustup run $RustToolchain cargo $($CargoArgs -join ' ')" -ForegroundColor Cyan
    & rustup run $RustToolchain cargo @CargoArgs
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo build failed with exit code $LASTEXITCODE."
    }
}

Assert-CommandAvailable -CommandName "rustup"
Assert-CommandAvailable -CommandName "cargo"

$repoRoot = Split-Path -Parent $PSScriptRoot
$controllerManifest = Join-Path $repoRoot 'Cargo.toml'
$tokenManifest = Join-Path $repoRoot 'token\Cargo.toml'
$absoluteArtifactsDir = Join-Path $repoRoot $ArtifactsDir
$absoluteTargetDir = Join-Path $repoRoot $TargetDir
$controllerTargetDir = Join-Path $absoluteTargetDir 'controller'
$tokenTargetDir = Join-Path $absoluteTargetDir 'token'
$resolvedWasmOpt = Resolve-WasmOpt

New-Item -ItemType Directory -Force -Path $absoluteArtifactsDir | Out-Null
New-Item -ItemType Directory -Force -Path $absoluteTargetDir | Out-Null
New-Item -ItemType Directory -Force -Path $controllerTargetDir | Out-Null
New-Item -ItemType Directory -Force -Path $tokenTargetDir | Out-Null

$previousRustFlags = $env:RUSTFLAGS

try {
    # Force a Wasm MVP-compatible build for older CosmWasm hosts such as rebel-2.
    $env:RUSTFLAGS = "-C target-cpu=mvp -C target-feature=+mutable-globals"

    Push-Location $repoRoot

    Invoke-Build -CargoArgs @(
        'build',
        '--release',
        '--lib',
        '--manifest-path', $controllerManifest,
        '--target', 'wasm32-unknown-unknown',
        '--target-dir', $controllerTargetDir
    )

    Invoke-Build -CargoArgs @(
        'build',
        '--release',
        '--lib',
        '--manifest-path', $tokenManifest,
        '--target', 'wasm32-unknown-unknown',
        '--target-dir', $tokenTargetDir
    )

    Copy-Item -LiteralPath (Join-Path $controllerTargetDir 'wasm32-unknown-unknown\release\isotropy_protocol.wasm') -Destination (Join-Path $absoluteArtifactsDir 'isotropy_protocol.wasm') -Force
    Copy-Item -LiteralPath (Join-Path $tokenTargetDir 'wasm32-unknown-unknown\release\isotropy_token.wasm') -Destination (Join-Path $absoluteArtifactsDir 'isotropy_token.wasm') -Force
    if ($null -ne $resolvedWasmOpt) {
        Invoke-WasmOpt -BinaryPath $resolvedWasmOpt -WasmPath (Join-Path $absoluteArtifactsDir 'isotropy_protocol.wasm')
        Invoke-WasmOpt -BinaryPath $resolvedWasmOpt -WasmPath (Join-Path $absoluteArtifactsDir 'isotropy_token.wasm')
    }
    New-GzipArtifact -SourcePath (Join-Path $absoluteArtifactsDir 'isotropy_protocol.wasm') -DestinationPath (Join-Path $absoluteArtifactsDir 'isotropy_protocol.wasm.gz')
    New-GzipArtifact -SourcePath (Join-Path $absoluteArtifactsDir 'isotropy_token.wasm') -DestinationPath (Join-Path $absoluteArtifactsDir 'isotropy_token.wasm.gz')

    Write-Host ""
    Write-Host "Artifacts refreshed:" -ForegroundColor Green
    Get-ChildItem -LiteralPath $absoluteArtifactsDir -File |
        Select-Object Name, Length, LastWriteTime |
        Format-Table -AutoSize

    Write-Host ""
    Write-Host "SHA256 checksums:" -ForegroundColor Green
    Get-FileHash -Algorithm SHA256 (Join-Path $absoluteArtifactsDir 'isotropy_protocol.wasm')
    Get-FileHash -Algorithm SHA256 (Join-Path $absoluteArtifactsDir 'isotropy_token.wasm')
}
finally {
    Pop-Location
    $env:RUSTFLAGS = $previousRustFlags
}
