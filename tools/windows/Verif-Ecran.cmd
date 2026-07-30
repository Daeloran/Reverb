@echo off
REM Lanceur des deux verifications visuelles restantes sur l'ecran du Kraken.
REM Le script attend une action a chaque etape : lancez-le quand vous etes dispo.
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-NoExit','-File','%~dp0Verif-Ecran.ps1'"
