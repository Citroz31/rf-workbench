@echo off
setlocal
cd /d "%~dp0"
if not exist "%~dp0rf-workbench.exe" (
  echo Extraire le paquet Windows complet depuis GitHub Releases.
  echo Le lanceur doit etre dans le meme dossier que rf-workbench.exe.
  pause
  exit /b 1
)
if not defined RF_WORKBENCH_PYTHON set "RF_WORKBENCH_PYTHON=python"
start "" "%~dp0rf-workbench.exe" %*
