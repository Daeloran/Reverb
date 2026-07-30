@echo off
REM Lanceur : ouvre une console PowerShell elevee et balaie les modes
REM d'affichage du Kraken. Regardez l'ecran pendant l'execution.
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-NoExit','-File','%~dp0Test-KrakenEcran.ps1'"
