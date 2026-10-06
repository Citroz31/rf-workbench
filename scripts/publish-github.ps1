param([string]$Name = 'rf-workbench', [switch]$Public)
$ErrorActionPreference = 'Stop'
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
        throw 'GitHub CLI (gh) doit être installé depuis https://cli.github.com/ pour publier ce dépôt.'
    }
    gh auth status
    if ($LASTEXITCODE -ne 0) { throw 'Se connecter avec gh auth login avant de publier.' }
    $owner = gh api user --jq '.login'
    if ($LASTEXITCODE -ne 0 -or -not $owner) { throw 'Compte GitHub introuvable.' }
    $status = git status --porcelain
    if ($status) { throw 'Le dépôt doit être propre et les modifications commitées avant publication.' }
    $visibility = if ($Public) { '--public' } else { '--private' }
    gh repo create "$owner/$Name" $visibility --source . --remote origin --push --description 'Native Rust RF instrumentation workbench with block diagrams, SCPI/VISA sessions and Python scripting'
    if ($LASTEXITCODE -ne 0) { throw 'Création/publication GitHub non confirmée. Vérifier gh repo view avant de réessayer.' }
    Write-Output "Dépôt publié : https://github.com/$owner/$Name"
} finally { Pop-Location }
