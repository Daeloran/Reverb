<#
    Balaie les modes d'affichage du Kraken en envoyant l'image en boucle,
    pour identifier lequel accepte une image de l'hote.

    A lancer soi-meme, en regardant l'ecran pendant l'execution : chaque
    phase est annoncee avant de commencer.

    Sans accents (UTF-8 sans BOM + PowerShell 5.1).
#>
param(
    [int[]] $Modes = @(2, 0, 1, 3, 4),
    [int]   $SecondesParMode = 10
)

$ErrorActionPreference = 'Continue'
$envoi = Join-Path $PSScriptRoot 'Send-KrakenImage.ps1'
if (-not (Test-Path $envoi)) { throw "Send-KrakenImage.ps1 introuvable a cote de ce script." }

$parMode = [Math]::Max(1, [int]($SecondesParMode / 0.7))

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host " Balayage des modes d'affichage du Kraken" -ForegroundColor Cyan
Write-Host " Regardez l'ecran. L'image attendue est une mire :" -ForegroundColor Cyan
Write-Host "     haut gauche ROUGE   |  haut droite VERT" -ForegroundColor Cyan
Write-Host "     bas gauche  BLEU    |  bas droite  BLANC" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan

foreach ($m in $Modes) {
    Write-Host ""
    Write-Host (">>> MODE $m  --  $SecondesParMode secondes, image envoyee en boucle") -ForegroundColor Yellow
    Write-Host "    (notez ce mode si quelque chose apparait)" -ForegroundColor DarkGray
    & $envoi -Mode $m -Repetitions $parMode -DelaiMs 200 -Motif quadrants 2>&1 |
        Where-Object { $_ -match 'envoi|ECHEC|erreur|mode d affichage' } |
        Select-Object -First 3 | ForEach-Object { "    $_" }
}

Write-Host ""
Write-Host "Balayage termine. Quel mode a affiche la mire ?" -ForegroundColor Green
Write-Host ""
