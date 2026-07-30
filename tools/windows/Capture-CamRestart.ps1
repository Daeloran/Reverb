<#
    Capture 0 -- redemarrage de NZXT CAM.

    Justification : les couleurs ne survivent pas au reboot. CAM rejoue donc
    toute la sequence d'initialisation a chaque demarrage. Cette capture
    contient l'init complet ET l'etat des 80 LED, en une fois.

    Sans accents : UTF-8 sans BOM + PowerShell 5.1.
#>
[CmdletBinding()]
param(
    [string] $Interface = '\\.\USBPcap5',
    [string] $Devices   = '6,7,8,9',
    [string] $OutDir    = $PSScriptRoot,
    [int]    $WaitAfterStart = 75,
    [string] $CamPath   = '',
    [string] $Label     = 'cible0-cam-restart'
)

$ErrorActionPreference = 'Stop'

$usbpcap = "C:\Program Files\USBPcap\USBPcapCMD.exe"
$stamp   = (Get-Date).ToString('yyyyMMdd-HHmmss')
$pcap    = Join-Path $OutDir "$Label-$stamp.pcap"
$journal = Join-Path $OutDir "$Label-$stamp.journal.txt"

function J {
    param([string] $Text)
    $line = '{0}  {1}' -f (Get-Date).ToString('HH:mm:ss.fff'), $Text
    Add-Content -Path $journal -Value $line -Encoding utf8
    Write-Host $line
}

# --- en-tete -------------------------------------------------------------

$hdr = @(
    "# Capture 0 -- redemarrage NZXT CAM",
    "# Date         : $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss K')",
    "# Interface    : $Interface   devices=$Devices",
    "#   6 = 1e71:300c Kraken Elite",
    "#   7 = 1e71:2019",
    "#   8 = 1e71:2012 (0E014044AB7664C25F063BD5)",
    "#   9 = 1e71:2012 (1101F021AA358489609AA5B2)",
    "#",
    "# ETAT LED CONNU (texte clair), 8 LED par ventilateur :",
    "#   bas gauche    rouge",
    "#   bas milieu    bleu",
    "#   bas droite    vert",
    "#   droit bas     rose",
    "#   droit milieu  jaune",
    "#   droit haut    magenta",
    "#   haut gauche   noir",
    "#   haut milieu   orange",
    "#   haut droite   blanc",
    "#   gauche        cyan",
    "#",
    "# Les couleurs ne persistent PAS au reboot : CAM les rejoue au demarrage.",
    ""
)
Set-Content -Path $journal -Value $hdr -Encoding utf8

# --- retrouver l'executable CAM avant de le tuer -------------------------

$camPath = $null
if ($CamPath -and (Test-Path $CamPath)) { $camPath = $CamPath }
if (-not $camPath) {
    foreach ($p in (Get-Process -Name 'NZXT CAM' -ErrorAction SilentlyContinue)) {
        try { if ($p.Path) { $camPath = $p.Path; break } } catch { }
    }
}
if (-not $camPath) {
    foreach ($guess in @("C:\Program Files\NZXT CAM\NZXT CAM.exe",
                         "$env:LOCALAPPDATA\Programs\NZXT CAM\NZXT CAM.exe")) {
        if (Test-Path $guess) { $camPath = $guess; break }
    }
}
if (-not $camPath) { throw "Executable NZXT CAM introuvable -- impossible de le relancer apres l'arret." }
J "executable CAM : $camPath"

# --- demarrage de la capture AVANT de toucher a CAM ----------------------

$args = @('-d', $Interface, '-o', $pcap, '-s', '65535', '-b', '134217728',
          '--devices', $Devices, '--inject-descriptors')

$proc = Start-Process -FilePath $usbpcap -ArgumentList $args -PassThru -WindowStyle Hidden `
    -RedirectStandardOutput (Join-Path $OutDir "cible0-$stamp.usbpcap.log") `
    -RedirectStandardError  (Join-Path $OutDir "cible0-$stamp.usbpcap.err")

Start-Sleep -Seconds 3
if ($proc.HasExited) {
    $err = Get-Content (Join-Path $OutDir "cible0-$stamp.usbpcap.err") -Raw -ErrorAction SilentlyContinue
    throw "USBPcapCMD s'est arrete immediatement (code $($proc.ExitCode)).`n$err"
}
J "debut capture (USBPcapCMD pid $($proc.Id)) -> $(Split-Path $pcap -Leaf)"

# 10 s de trafic de fond, CAM encore vivant : sert de reference pour
# distinguer le polling du trafic utile.
J "10 s de trafic de fond avec CAM actif"
Start-Sleep -Seconds 10

# --- arret de CAM --------------------------------------------------------

J "ARRET de NZXT CAM"
Get-Process -Name 'NZXT CAM', 'cam_helper' -ErrorAction SilentlyContinue |
    ForEach-Object {
        J "  kill $($_.ProcessName) pid $($_.Id)"
        Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
    }

J "CAM arrete -- 15 s d observation (les LED s eteignent-elles ?)"
Start-Sleep -Seconds 15

# --- relance de CAM ------------------------------------------------------

J "RELANCE de NZXT CAM"
Start-Process -FilePath $camPath | Out-Null
J "CAM relance -- $WaitAfterStart s pour l init complet et la reapplication des couleurs"
Start-Sleep -Seconds $WaitAfterStart

# --- silence final -------------------------------------------------------

J "silence final 10 s"
Start-Sleep -Seconds 10
J "fin capture"

Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 3

# --- bilan ---------------------------------------------------------------

if (Test-Path $pcap) {
    $size = (Get-Item $pcap).Length
    J ("pcap : {0:N0} octets ({1:N2} Mo)" -f $size, ($size / 1MB))
} else {
    J "ERREUR : aucun pcap produit"
}
J "journal : $journal"
