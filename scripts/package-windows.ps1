#requires -Version 7.2
[CmdletBinding()]
param(
    [string]$BinaryPath = '',
    [string]$OutputDirectory = '',
    [string]$Version = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repositoryRoot = Split-Path $PSScriptRoot -Parent
if (-not $BinaryPath) {
    $BinaryPath = Join-Path $repositoryRoot 'target/release/rf-workbench.exe'
}
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $repositoryRoot 'dist/portable'
}
if (-not $Version) {
    $versionLine = Get-Content -LiteralPath (Join-Path $repositoryRoot 'Cargo.toml') |
        Where-Object { $_ -match '^version\s*=\s*"([^"]+)"' } | Select-Object -First 1
    if (-not $versionLine -or $versionLine -notmatch '^version\s*=\s*"([^"]+)"') {
        throw 'La version du workspace Cargo est introuvable. Fournir -Version.'
    }
    $Version = $Matches[1]
}
if ($Version -notmatch '^[A-Za-z0-9][A-Za-z0-9._+-]*$') {
    throw 'Version invalide : utiliser une version sans espace ni séparateur de chemin.'
}

$binary = Get-Item -LiteralPath $BinaryPath -ErrorAction Stop
if ($binary.PSIsContainer -or $binary.Extension -ine '.exe' -or $binary.Length -le 0) {
    throw 'BinaryPath doit désigner un exécutable .exe non vide déjà compilé.'
}
$sourceBinaryHash = (Get-FileHash -LiteralPath $binary.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
$licensePath = Join-Path $repositoryRoot 'LICENSE'
$guidePath = Join-Path $repositoryRoot 'docs/Guide-utilisateur.pdf'
$exampleFiles = @(Get-ChildItem -LiteralPath (Join-Path $repositoryRoot 'examples') -Filter '*.rfbench' -File)
foreach ($requiredPath in @($licensePath, $guidePath)) {
    if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf) -or
        (Get-Item -LiteralPath $requiredPath).Length -le 0) {
        throw "Document requis absent ou vide : $requiredPath"
    }
}
if ($exampleFiles.Count -eq 0) {
    throw 'Aucun exemple .rfbench : la livraison serait incomplète.'
}

New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$outputRoot = (Resolve-Path -LiteralPath $OutputDirectory).Path
$stagePath = Join-Path $outputRoot ".rf-workbench-package-$(Get-Random)"
$verificationPath = Join-Path $outputRoot ".rf-workbench-verify-$(Get-Random)"
$archivePath = Join-Path $outputRoot 'RF-Workbench-Windows-x64-portable.zip'
$standalonePath = Join-Path $outputRoot 'rf-workbench.exe'

try {
    New-Item -ItemType Directory -Path $stagePath -ErrorAction Stop | Out-Null
    $stagePath = (Resolve-Path -LiteralPath $stagePath).Path
    New-Item -ItemType Directory -Path (Join-Path $stagePath 'docs'), (Join-Path $stagePath 'examples') | Out-Null
    Copy-Item -LiteralPath $binary.FullName -Destination (Join-Path $stagePath 'rf-workbench.exe')
    Copy-Item -LiteralPath $licensePath -Destination (Join-Path $stagePath 'LICENSE')
    Copy-Item -LiteralPath $guidePath -Destination (Join-Path $stagePath 'docs/Guide-utilisateur.pdf')
    foreach ($example in $exampleFiles) {
        if ($example.Length -le 0) { throw "Exemple vide : $($example.Name)" }
        Copy-Item -LiteralPath $example.FullName -Destination (Join-Path $stagePath "examples/$($example.Name)")
    }

    @"
RF Workbench $Version - Windows x64 - application portable

DÉMARRAGE
1. Extraire entièrement RF-Workbench-Windows-x64-portable.zip dans un dossier de votre compte,
   par exemple Documents\RF Workbench.
2. Ouvrir ce dossier : rf-workbench.exe est directement à sa racine.
3. Double-cliquer sur rf-workbench.exe (type Application si Windows masque .exe).
   Aucun CMD, terminal, VS Code ni installation n'est nécessaire au lancement.
4. À l'accueil, ouvrir un projet de examples (PNA-X.rfbench, PA-36-38GHz.rfbench,
   QAM16-AWGN.rfbench, LNA-MAAL-FR1245.rfbench, Corechip-CGY2170YHV-C1.rfbench, PNA-X-N5245B-4ports.rfbench),
   conserver Simulation, puis utiliser Exécuter.

La simulation intégrée de ces exemples fonctionne sans Rust, Python ni VISA.
Le programme s'exécute localement. Le téléchargement initial nécessite Internet ;
la simulation ne nécessite ensuite ni compte, serveur ni connexion Internet.
Vos projets .rfbench et exports sont enregistrés aux emplacements que vous choisissez.
L'application ne demande pas d'élévation administrateur et ce paquet ne contient
ni installateur, service, tâche planifiée ni modification du registre.

CONTRAINTES DE VOTRE ENTREPRISE
L'exécutable n'est pas signé numériquement actuellement. Les règles de votre PC
peuvent donc refuser son téléchargement ou son exécution, même sans installation.
En cas de blocage, suppression, quarantaine ou demande administrateur, communiquer
le message exact et SHA256SUMS.txt à l'IT pour validation. Ne désactiver aucune
protection. Le caractère portable ne remplace pas l'autorisation de votre entreprise.

FONCTIONS OPTIONNELLES
- Scripts Python : choisir un Python 3.10+ déjà autorisé dans l'onglet Python.
- Instruments GPIB/USB et LAN VISA INSTR : runtime VISA x64 du constructeur requis.
  Utiliser celui déjà installé ou demander sa mise à disposition à l'IT.
- Le SCPI TCP SOCKET natif n'utilise pas VISA. Le mode Matériel réel communique
  avec les équipements/adresses choisis ; la simulation reste le premier essai.
- Aucun instrument physique n'a été validé dans cette livraison du prototype.

CONTENU
rf-workbench.exe            Application Windows x64
docs\Guide-utilisateur.pdf Guide détaillé, incluant le démarrage direct
examples\*.rfbench         Bancs d'exemple
LICENSE                    Licence MIT
SHA256SUMS.txt              Empreintes des fichiers contenus dans ce ZIP

Les fichiers Source code de GitHub contiennent les sources, pas ce programme.
Téléchargements Windows : https://github.com/Citroz31/rf-workbench/releases/latest
"@ | Set-Content -LiteralPath (Join-Path $stagePath 'LISEZ-MOI.txt') -Encoding utf8

    $contentChecksums = foreach ($file in Get-ChildItem -LiteralPath $stagePath -Recurse -File | Sort-Object FullName) {
        $relativeName = $file.FullName.Substring($stagePath.Length + 1).Replace('\', '/')
        $hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash  $relativeName"
    }
    $contentChecksums | Set-Content -LiteralPath (Join-Path $stagePath 'SHA256SUMS.txt') -Encoding ascii

    # Archive directory contents, so the EXE is at the ZIP root, without a wrapper folder.
    Compress-Archive -Path (Join-Path $stagePath '*') -DestinationPath $archivePath -CompressionLevel Optimal -Force
    if ((Get-Item -LiteralPath $archivePath).Length -le 0) { throw 'ZIP vide.' }
    Expand-Archive -LiteralPath $archivePath -DestinationPath $verificationPath
    $verificationPath = (Resolve-Path -LiteralPath $verificationPath).Path
    $extractedBinary = Join-Path $verificationPath 'rf-workbench.exe'
    if (-not (Test-Path -LiteralPath $extractedBinary -PathType Leaf) -or
        (Get-Item -LiteralPath $extractedBinary).Length -le 0) {
        throw 'ZIP invalide : rf-workbench.exe doit être non vide et directement à la racine.'
    }
    if ((Get-FileHash -LiteralPath $extractedBinary -Algorithm SHA256).Hash.ToLowerInvariant() -ne $sourceBinaryHash) {
        throw 'ZIP invalide : le binaire extrait ne correspond pas au binaire compilé.'
    }
    $expectedFiles = @(Get-ChildItem -LiteralPath $stagePath -Recurse -File | ForEach-Object {
        $_.FullName.Substring($stagePath.Length + 1)
    } | Sort-Object)
    $actualFiles = @(Get-ChildItem -LiteralPath $verificationPath -Recurse -File | ForEach-Object {
        $_.FullName.Substring($verificationPath.Length + 1)
    } | Sort-Object)
    if (Compare-Object -ReferenceObject $expectedFiles -DifferenceObject $actualFiles) {
        throw 'ZIP invalide : la liste des fichiers extraits diffère de la livraison préparée.'
    }
    foreach ($relativeName in $expectedFiles) {
        $before = (Get-FileHash -LiteralPath (Join-Path $stagePath $relativeName) -Algorithm SHA256).Hash
        $after = (Get-FileHash -LiteralPath (Join-Path $verificationPath $relativeName) -Algorithm SHA256).Hash
        if ($before -ne $after) { throw "ZIP corrompu : $relativeName" }
    }

    if ($binary.FullName -ine $standalonePath) {
        Copy-Item -LiteralPath $binary.FullName -Destination $standalonePath -Force
    }
    if ((Get-FileHash -LiteralPath $standalonePath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $sourceBinaryHash) {
        throw 'Le téléchargement EXE seul ne correspond pas au binaire compilé.'
    }
    "$sourceBinaryHash  rf-workbench.exe" |
        Set-Content -LiteralPath (Join-Path $outputRoot 'rf-workbench.exe.sha256') -Encoding ascii
    $releaseChecksums = foreach ($asset in @($standalonePath, $archivePath)) {
        $hash = (Get-FileHash -LiteralPath $asset -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash  $(Split-Path $asset -Leaf)"
    }
    $releaseChecksums | Set-Content -LiteralPath (Join-Path $outputRoot 'SHA256SUMS.txt') -Encoding ascii
    [pscustomobject]@{
        Version = $Version
        Binary = $standalonePath
        BinaryBytes = (Get-Item -LiteralPath $standalonePath).Length
        BinarySHA256 = $sourceBinaryHash
        Archive = $archivePath
        Checksums = Join-Path $outputRoot 'SHA256SUMS.txt'
        VerifiedFiles = $expectedFiles.Count
    }
}
finally {
    foreach ($temporaryPath in @($stagePath, $verificationPath)) {
        if (Test-Path -LiteralPath $temporaryPath) {
            $resolvedTemporaryPath = (Resolve-Path -LiteralPath $temporaryPath).Path
            # Both recursive cleanup targets must be children created in OutputDirectory.
            if ((Split-Path $resolvedTemporaryPath -Parent) -ine $outputRoot -or
                (Split-Path $resolvedTemporaryPath -Leaf) -notlike '.rf-workbench-*') {
                throw "Nettoyage refusé hors du dossier de sortie : $resolvedTemporaryPath"
            }
            Remove-Item -LiteralPath $resolvedTemporaryPath -Recurse -Force
        }
    }
}
