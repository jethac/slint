# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

[CmdletBinding()]
param(
    [switch]$Parity,
    [string]$TokenSource
)

$ErrorActionPreference = 'Stop'
$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '../../..')).Path
$artifactDirectory = Join-Path $repositoryRoot 'target/material-preview'
$variables = @('FIELDNOTES_SCREENSHOTS', 'MATERIAL_GALLERY_SCREENSHOTS',
    'SLINT_TEST_FILTER', 'PARITY_REQUIRE_REFS', 'PARITY_ARTIFACT_DIR', 'M3_TOKENS_SRC', 'RUST_MIN_STACK')
$previousValues = @{}
foreach ($name in $variables) {
    $previousValues[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}

function Invoke-CargoCheck {
    param([string[]]$CargoArguments)
    & cargo @CargoArguments
    if ($LASTEXITCODE -ne 0) {
        throw "cargo $($CargoArguments -join ' ') failed with exit code $LASTEXITCODE"
    }
}

function Invoke-ParityChecks {
    $caseRoot = (Resolve-Path tests/screenshots/cases).Path
    $cases = [string[]](Get-ChildItem tests/screenshots/cases/material -Filter '*.slint' -Recurse |
        ForEach-Object { $_.FullName.Substring($caseRoot.Length + 1).Replace('\', '/') })
    [Array]::Sort($cases, [StringComparer]::Ordinal)
    if ($cases.Count -eq 0) { throw 'No Material parity cases found' }
    $failed = $false
    for ($shard = 0; $shard -lt 6; $shard++) {
        $selected = for ($index = $shard; $index -lt $cases.Count; $index += 6) { $cases[$index] }
        if (!$selected) { continue }
        $env:SLINT_TEST_FILTER = $selected -join ','
        $messages = & cargo --config profile.dev.package.i-slint-compiler.opt-level=3 --config profile.dev.package.test-driver-screenshots.debug=0 test --manifest-path tests/Cargo.toml -p test-driver-screenshots --no-run --message-format=json-render-diagnostics
        if ($LASTEXITCODE -ne 0) { throw "Parity shard $shard failed to compile" }
        $binary = $null
        foreach ($line in $messages) {
            $message = $line | ConvertFrom-Json
            if ($message.reason -eq 'compiler-artifact' -and $message.profile.test -and
                $message.target.name -eq 'test-driver-screenshot' -and $message.executable) {
                $binary = $message.executable
            }
        }
        if (!$binary) { throw "No test executable for parity shard $shard" }
        $listed = & $binary --list
        if ($LASTEXITCODE -ne 0) { throw 'Cannot list parity tests' }
        $names = @(foreach ($line in $listed) { if ($line -match '^(.+): test$') { $Matches[1] } })
        if ($names.Count -eq 0) { throw 'No parity tests found' }
        Push-Location tests
        try {
            for ($start = 0; $start -lt $names.Count; $start += 40) {
                $end = [Math]::Min($start + 40, $names.Count) - 1
                $batch = @($names[$start..$end])
                & $binary --test-threads=4 --exact @batch
                if ($LASTEXITCODE -ne 0) { $failed = $true }
            }
        } finally { Pop-Location }
    }
    if ($failed) { throw 'Material parity tests failed; inspect the batch results and artifacts' }
}

Push-Location $repositoryRoot
try {
    $env:FIELDNOTES_SCREENSHOTS = Join-Path $artifactDirectory 'fieldnotes'
    $env:MATERIAL_GALLERY_SCREENSHOTS = Join-Path $artifactDirectory 'gallery'
    if ($TokenSource) {
        $env:M3_TOKENS_SRC = (Resolve-Path -LiteralPath $TokenSource).Path
    }
    $manifest = 'ui-libraries/material/Cargo.toml'
    Invoke-CargoCheck -CargoArguments @('test', '--manifest-path', $manifest, '-p', 'material-token-generator', '-p', 'material-parity-generator')
    Invoke-CargoCheck -CargoArguments @('run', '--manifest-path', $manifest, '-p', 'material-token-generator', '--', 'check')
    Invoke-CargoCheck -CargoArguments @('run', '--manifest-path', $manifest, '-p', 'material-parity-generator', '--', '--check')
    Invoke-CargoCheck -CargoArguments @('test', '--manifest-path', $manifest, '-p', 'material-gallery', '-p', 'material-fieldnotes')
    if ($Parity) {
        if (!$env:RUST_MIN_STACK) { $env:RUST_MIN_STACK = '16777216' }
        $env:SLINT_TEST_FILTER = 'material'
        $env:PARITY_REQUIRE_REFS = '1'
        $env:PARITY_ARTIFACT_DIR = Join-Path $artifactDirectory 'parity'
        Invoke-ParityChecks
    }
    Write-Host "Preview images: $artifactDirectory"
} finally {
    foreach ($name in $variables) {
        [Environment]::SetEnvironmentVariable($name, $previousValues[$name], 'Process')
    }
    Pop-Location
}
