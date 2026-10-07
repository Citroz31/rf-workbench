# Windows portable, lancement local

Le téléchargement prêt à utiliser est disponible dans les **Assets** de la [dernière release GitHub](https://github.com/Citroz31/rf-workbench/releases/latest). Le bouton **Code → Download ZIP** et les fichiers **Source code** contiennent les sources ; ils ne contiennent pas d'exécutable compilé.

## Télécharger et ouvrir

1. Télécharger [RF-Workbench-Windows-x64-portable.zip](https://github.com/Citroz31/rf-workbench/releases/latest/download/RF-Workbench-Windows-x64-portable.zip).
2. Extraire entièrement le ZIP dans un dossier accessible à votre compte, par exemple `Documents\RF Workbench`.
3. Dans le dossier extrait, double-cliquer sur **rf-workbench.exe**, directement à la racine. Si Windows masque les extensions, son nom apparaît comme **rf-workbench**, avec le type **Application**.
4. À l'accueil, ouvrir un projet du dossier `examples`, conserver **Simulation**, puis cliquer sur **Exécuter**. Les exemples `PNA-X.rfbench`, `PA-36-38GHz.rfbench` et `QAM16-AWGN.rfbench` utilisent les simulations intégrées.

Le ZIP a cette organisation :

```text
rf-workbench.exe
LISEZ-MOI.txt
LICENSE
SHA256SUMS.txt
docs/
  Guide-utilisateur.pdf
examples/
  PNA-X.rfbench
  PA-36-38GHz.rfbench
  QAM16-AWGN.rfbench
```

On peut également télécharger [rf-workbench.exe seul](https://github.com/Citroz31/rf-workbench/releases/latest/download/rf-workbench.exe). Il ouvre l'interface sans fichier annexe ; les démonstrations intégrées sont accessibles depuis l'application. Le ZIP inclut en plus le guide et les projets enregistrés.

Le démarrage direct remplace les anciennes instructions faisant appel au CMD dans le guide PDF. Aucun CMD, VS Code ou terminal n'est requis pour utiliser le paquet portable.

## Fonctionnement et droits

RF Workbench est une application native **Windows x64**. Les exemples simulés intégrés fonctionnent sans Rust, sans Python, sans runtime VISA et sans installation. Après le téléchargement, la simulation fonctionne localement sans compte, serveur ou connexion Internet. Enregistrez vos projets `.rfbench` et exports dans un dossier de votre compte.

L'application ne demande pas d'élévation administrateur. Le paquet ne contient ni installateur, service, tâche planifiée, modification du registre ou inscription automatique d'association de fichiers. Il n'est actuellement **pas signé numériquement** : la politique de votre entreprise peut donc refuser son téléchargement ou son exécution. Le mode portable ne garantit pas l'autorisation de l'IT.

Depuis 0.5.1, le manifeste Windows indique explicitement `asInvoker` et `uiAccess=false`. Le dossier initial des projets, préférences, Studio, catalogue DUT, exports CSV et binaires est `%LOCALAPPDATA%\RF Workbench`, affiché à l'accueil. Le programme ne demande pas d'écrire dans son dossier d'installation ou dans le répertoire courant. Un dossier de données différent peut être choisi avec `RF_WORKBENCH_DATA_DIR` (chemin absolu). Les projets restent enregistrables où vous le souhaitez. Les anciens fichiers enregistrés ailleurs peuvent être ouverts manuellement ; ils ne sont ni migrés ni écrasés automatiquement. Si le dossier utilisateur est inaccessible, l'application affiche un avertissement et utilise un dossier temporaire pour la session : sauvegarder vos projets ailleurs avant de quitter.

Si le fichier est absent après extraction, bloqué, mis en quarantaine ou supprimé, conservez le message exact et transmettez à l'IT le lien de la release et l'empreinte SHA256. Ne désactivez aucune protection. Un message d'installation ou d'élévation n'est pas une étape normale de ce paquet.

## Python et équipements, uniquement si nécessaires

- Les blocs Python demandent un **Python 3.10+** déjà installé et autorisé, à sélectionner dans l'onglet Python. Les scripts exécutés disposent des permissions du compte utilisateur.
- Les instruments **GPIB/USB** et **LAN VISA INSTR** demandent un runtime **VISA x64** autorisé. Utiliser celui déjà présent ; sa mise à disposition éventuelle passe par l'IT.
- Le transport **SCPI TCP SOCKET natif** n'utilise pas VISA. Les fonctionnalités matérielles communiquent avec les adresses et instruments configurés lorsque **Matériel réel** est activé. Le premier essai reste en simulation.

Le prototype ne constitue pas une validation d'un instrument physique ou de performances de métrologie. Consulter le guide PDF et les matrices de capacités pour les fonctions disponibles et leurs limites.

## Vérifier le téléchargement

Le fichier `SHA256SUMS.txt` joint à la release donne les empreintes de `rf-workbench.exe` et `RF-Workbench-Windows-x64-portable.zip`. Dans un terminal PowerShell déjà disponible, on peut calculer l'empreinte sans installation :

```powershell
Get-FileHash -LiteralPath '.\rf-workbench.exe' -Algorithm SHA256
Get-FileHash -LiteralPath '.\RF-Workbench-Windows-x64-portable.zip' -Algorithm SHA256
```

Le ZIP contient également les empreintes du guide, des exemples et du binaire. L'empreinte identifie les octets téléchargés ; elle ne remplace pas une signature numérique ou la validation IT.

## Produire une livraison, pour les développeurs

Le script d'empaquetage exige PowerShell 7.2+ et un binaire déjà compilé. Il ne compile pas, n'installe rien, ne modifie pas le registre et ne contacte aucun service :

```powershell
./scripts/package-windows.ps1 -BinaryPath target/release/rf-workbench.exe -OutputDirectory dist/portable
```

`-Version` est facultatif ; sans cette option, la version du workspace Cargo est utilisée. Le script refuse un EXE absent ou vide, ne copie que le binaire, les documents et exemples sélectionnés, génère les empreintes SHA256, extrait le ZIP et vérifie la présence de l'EXE à sa racine et les empreintes de tous les fichiers extraits.

Le workflow `release.yml` construit et teste sous Windows pour les tags `v*`. Il peut aussi être déclenché manuellement avec un tag de version existant. Il exécute les tests Rust avec `--locked`, compile en release, vérifie les autotests et une simulation DSP dans le vrai EXE, empaquette, puis publie et contrôle la présence des quatre assets GitHub. Une nouvelle release reste en brouillon jusqu'au contrôle des assets. Il télécharge ensuite publiquement l'EXE et le ZIP sans authentification pour vérifier les tailles, empreintes et contenu. Les dépendances de développement et l'accès réseau concernent uniquement cette construction ; ils ne sont pas requis sur le PC qui utilise le paquet portable.
