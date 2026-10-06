param([switch]$Build)
$ErrorActionPreference = 'Stop'
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw 'Rust formatting failed' }
    cargo clippy --workspace --all-targets --locked -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'Rust lint failed' }
    cargo test --workspace --locked
    if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed' }
    cargo test -p rf-runtime --test python_integration --locked -- --ignored
    if ($LASTEXITCODE -ne 0) { throw 'Rust/Python tests failed' }
    Push-Location python
    try {
        foreach ($argsForUv in @(@('ruff','check','.'), @('ruff','format','--check','.'), @('ty','check'), @('basedpyright'), @('pytest','--cov','--cov-branch'))) {
            uv run @argsForUv
            if ($LASTEXITCODE -ne 0) { throw "Python check failed: $argsForUv" }
        }
    } finally { Pop-Location }
    if ($Build) {
        cargo build --release -p rf-workbench --locked
        if ($LASTEXITCODE -ne 0) { throw 'Build failed' }
    }
} finally { Pop-Location }
