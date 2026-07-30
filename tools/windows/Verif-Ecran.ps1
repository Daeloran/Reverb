<#
    Verifications visuelles restantes sur l'ecran du Kraken.

    Le script attend une action de votre part entre chaque etape : rien ne
    demarre tant que vous n'etes pas devant l'ecran.

    Sans accents (UTF-8 sans BOM + PowerShell 5.1).
#>
$ErrorActionPreference = 'Continue'
$envoi = Join-Path $PSScriptRoot 'Send-KrakenImage.ps1'
if (-not (Test-Path $envoi)) { throw "Send-KrakenImage.ps1 introuvable a cote de ce script." }

function Pause-Ici { param([string] $Texte)
    Write-Host ""
    Write-Host $Texte -ForegroundColor White
    Write-Host "    -> Entree pour continuer" -ForegroundColor DarkGray
    [void](Read-Host)
}

Write-Host ""
Write-Host "==============================================================" -ForegroundColor Cyan
Write-Host " Deux verifications, environ 2 minutes." -ForegroundColor Cyan
Write-Host " Rien ne demarre sans votre accord." -ForegroundColor Cyan
Write-Host "==============================================================" -ForegroundColor Cyan

# --------------------------------------------------------------------
Pause-Ici "ETAPE 1 sur 2 -- ordre des couleurs.`n    Je vais afficher une mire pendant 25 secondes.`n    Regardez OU se trouve le ROUGE et ou se trouve le BLEU."

Write-Host "  affichage en cours..." -ForegroundColor Yellow
& $envoi -Mode 2 -Repetitions 45 -DelaiMs 300 -Motif quadrants 2>&1 |
    Where-Object { $_ -match 'ECHEC|erreur' } | ForEach-Object { "    $_" }

Write-Host ""
Write-Host "  Attendu si l'ordre BGR de la spec est correct :" -ForegroundColor Cyan
Write-Host "      haut gauche ROUGE   |  haut droite VERT" -ForegroundColor Cyan
Write-Host "      bas gauche  BLEU    |  bas droite  BLANC" -ForegroundColor Cyan
Write-Host "  Si ROUGE et BLEU sont echanges, l'ecran est en RGB." -ForegroundColor Cyan

# --------------------------------------------------------------------
Pause-Ici "ETAPE 2 sur 2 -- luminosite.`n    Je vais alterner luminosite MINI et MAXI toutes les 6 secondes,`n    quatre fois, en maintenant l'image affichee.`n    Regardez si l'ecran change d'intensite."

$niveaux = @(5, 100, 5, 100)
foreach ($n in $niveaux) {
    Write-Host ("  >>> luminosite = {0}" -f $n) -ForegroundColor Yellow
    # L'image doit etre reemise : le firmware reprend la main au bout de ~30 s.
    & $envoi -Mode 2 -Repetitions 12 -DelaiMs 100 -Motif quadrants 2>&1 |
        Where-Object { $_ -match 'ECHEC|erreur' } | ForEach-Object { "    $_" }
    & (Join-Path $PSScriptRoot 'Set-KrakenScreen.ps1') -Luminosite $n 2>&1 |
        Where-Object { $_ -match 'ECHEC' } | ForEach-Object { "    $_" }
    Start-Sleep -Seconds 3
}

Write-Host ""
Write-Host "==============================================================" -ForegroundColor Green
Write-Host " Termine. Deux reponses a rapporter :" -ForegroundColor Green
Write-Host "   1. Le ROUGE etait en haut a gauche, ou en bas a gauche ?" -ForegroundColor Green
Write-Host "   2. La luminosite a-t-elle change, oui ou non ?" -ForegroundColor Green
Write-Host "==============================================================" -ForegroundColor Green
Write-Host ""
