<#
    Capture guidee USB pour le reverse engineering RGB NZXT.

    Lance USBPcapCMD sur l'interface et les devices choisis, puis deroule la
    sequence d'actions en ecrivant un journal horodate synchronise.

    La sequence de la cible 1 a ete refondue apres l'analyse de la capture 0 :
    elle vise maintenant les questions ouvertes de SPEC-PROTOCOLE-NZXT.md,
    en priorite le pilotage LED par LED.

    NOTE: chaines sans accents (UTF-8 sans BOM + PowerShell 5.1).

    Usage (fenetre PowerShell ADMIN) :
        .\Run-Capture.ps1 -Target 1
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet(1, 2)]
    [int] $Target,

    [string] $Interface = '\\.\USBPcap5',

    # Par defaut : cible 1 -> controleurs RGB seuls (on exclut le Kraken et ses
    # 655 Mo de streaming LCD). Cible 2 -> le Kraken seul.
    [string] $Devices = '',

    [string] $OutDir = $PSScriptRoot,

    [int] $GuardSeconds = 5
)

$ErrorActionPreference = 'Stop'

# --------------------------------------------------------------------------
# Sequences d'actions
# --------------------------------------------------------------------------

$sequences = @{
    1 = @(
        @{ Do = 'Couleur FIXE ROUGE sur TOUS les canaux d un coup'
           Look = 'une seule trame masque 0x3f, ou six trames ? -> role de l offset 3' }

        @{ Do = 'Couleur FIXE BLEUE sur UN SEUL ventilateur (bas gauche)'
           Look = 'confirme que le masque offset 2 adresse bien un canal' }

        @{ Do = 'Couleur FIXE BLEUE sur UN SEUL autre ventilateur (droit bas)'
           Look = 'confirme le bit de canal attendu 0x08' }

        @{ Do = 'LUMINOSITE au MAXIMUM (100 %)'
           Look = 'l octet [5] passe-t-il de 0x32 a 0x64 ? -> prouve la luminosite' }

        @{ Do = 'LUMINOSITE au MINIMUM'
           Look = 'confirmation de l octet [5]' }

        @{ Do = 'Mode BREATHING rouge'
           Look = 'octet de mode [4], et nombre de triplets envoyes' }

        @{ Do = 'Mode SPECTRUM WAVE'
           Look = 'octet de mode [4] -- mode genere par le controleur' }

        @{ Do = 'Changer la VITESSE de l animation'
           Look = 'octet [6] : vitesse et/ou direction' }

        @{ Do = 'Changer la DIRECTION de l animation si CAM le permet'
           Look = 'isole le champ direction dans l octet [6]' }

        @{ Do = 'Mode MULTICOLORE sur UN SEUL ventilateur -- Alternating ou Marquee, avec 3 couleurs BIEN DISTINCTES (rouge, vert, bleu)'
           Look = 'LA QUESTION CENTRALE : disposition des triplets GRB successifs a partir de l offset 7' }

        @{ Do = 'Si CAM le permet, colorer des LED INDIVIDUELLES du meme ventilateur differemment'
           Look = 'preuve directe de l adressage LED par LED' }

        @{ Do = 'Repasser en FIXE MAGENTA sur tous les canaux'
           Look = 'etat final connu' }
    )
    2 = @(
        @{ Do = 'Afficher une COULEUR UNIE sur l ecran';      Look = 'trame LCD minimale' }
        @{ Do = 'Afficher la TEMPERATURE DU LIQUIDE';         Look = 'mode integre, pas d image envoyee' }
        @{ Do = 'Regler la LUMINOSITE de l ecran';            Look = 'octet de luminosite' }
        @{ Do = 'Changer l ORIENTATION';                      Look = 'octet d orientation' }
        @{ Do = 'Charger une IMAGE FIXE';                     Look = 'transfert bulk complet' }
    )
}

$labels        = @{ 1 = 'cible1-modes-nzxt';    2 = 'cible2-ecran-kraken' }
$defaultDevices = @{ 1 = '7,8,9';               2 = '6' }

$actions = $sequences[$Target]
$label   = $labels[$Target]
if (-not $Devices) { $Devices = $defaultDevices[$Target] }

# --------------------------------------------------------------------------
# Prerequis
# --------------------------------------------------------------------------

$isAdmin = ([Security.Principal.WindowsPrincipal] `
    [Security.Principal.WindowsIdentity]::GetCurrent()
).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) { throw "Fenetre non elevee. USBPcap exige des droits administrateur." }

$usbpcap = "C:\Program Files\USBPcap\USBPcapCMD.exe"
if (-not (Test-Path $usbpcap)) { throw "USBPcapCMD.exe introuvable." }
if (-not (Test-Path $OutDir)) { New-Item -ItemType Directory -Path $OutDir | Out-Null }

$stamp   = (Get-Date).ToString('yyyyMMdd-HHmmss')
$pcap    = Join-Path $OutDir "$label-$stamp.pcap"
$journal = Join-Path $OutDir "$label-$stamp.journal.txt"

function Write-Journal {
    param([string] $Text, [switch] $NoTimestamp)
    if ($NoTimestamp) { $line = $Text }
    else { $line = '{0}  {1}' -f (Get-Date).ToString('HH:mm:ss.fff'), $Text }
    Add-Content -Path $journal -Value $line -Encoding utf8
    return $line
}

# --------------------------------------------------------------------------
# En-tete du journal
# --------------------------------------------------------------------------

$installed = Get-ItemProperty @(
    'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
) -ErrorAction SilentlyContinue

$camVersion = 'inconnue'
$cam = $installed | Where-Object { $_.DisplayName -match 'NZXT CAM' } | Select-Object -First 1
if ($cam) { $camVersion = "$($cam.DisplayName) / $($cam.DisplayVersion)" }

@(
    "# Capture cible $Target -- $label",
    "# Date       : $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss K')",
    "# NZXT CAM   : $camVersion",
    "# Interface  : $Interface   devices=$Devices",
    "#   6 = 1e71:300c Kraken | 7 = 1e71:2019 | 8,9 = 1e71:2012",
    "# pcap       : $(Split-Path $pcap -Leaf)",
    "#",
    "# Horodatage local, meme horloge que le pcap.",
    ""
) | ForEach-Object { Write-Journal -NoTimestamp -Text $_ } | Out-Null

# --------------------------------------------------------------------------
# Demarrage
# --------------------------------------------------------------------------

Write-Host ""
Write-Host "=== CAPTURE CIBLE $Target -- $label ===" -ForegroundColor Cyan
Write-Host "  interface : $Interface   devices=$Devices"
Write-Host "  pcap      : $pcap"
Write-Host ""

$args = @('-d', $Interface, '-o', $pcap, '-s', '65535', '-b', '134217728',
          '--devices', $Devices, '--inject-descriptors')

$proc = Start-Process -FilePath $usbpcap -ArgumentList $args -PassThru -WindowStyle Hidden `
    -RedirectStandardOutput (Join-Path $OutDir "$label-$stamp.usbpcap.log") `
    -RedirectStandardError  (Join-Path $OutDir "$label-$stamp.usbpcap.err")

Start-Sleep -Seconds 3
if ($proc.HasExited) {
    $err = Get-Content (Join-Path $OutDir "$label-$stamp.usbpcap.err") -Raw -ErrorAction SilentlyContinue
    throw "USBPcapCMD s'est arrete immediatement (code $($proc.ExitCode)).`n$err"
}
Write-Journal -Text "debut capture (pid $($proc.Id))" | Out-Null
Write-Host "Capture demarree (pid $($proc.Id))." -ForegroundColor Green

# --- trafic de fond -------------------------------------------------------

Write-Host ""
Write-Host "ACTION 1 : NE TOUCHE A RIEN pendant 10 secondes." -ForegroundColor Yellow
Write-Host "           Enregistre le polling de fond (62 01 chaque seconde)."
Write-Journal -Text "action 1 - debut 10 s d inaction" | Out-Null
for ($i = 10; $i -gt 0; $i--) {
    Write-Host "`r           $i s restantes  " -NoNewline
    Start-Sleep -Seconds 1
}
Write-Host "`r           trafic de fond enregistre.        "
Write-Journal -Text "action 1 - fin 10 s d inaction" | Out-Null

# --- boucle des actions ---------------------------------------------------

$n = 1
foreach ($a in $actions) {
    $n++
    Write-Host ""
    Write-Host ("-" * 72) -ForegroundColor DarkGray
    Write-Host "ACTION $n : $($a.Do)" -ForegroundColor Yellow
    Write-Host "  on cherche : $($a.Look)" -ForegroundColor DarkGray
    Write-Host ""
    Write-Host "  ESPACE juste apres avoir valide dans CAM   |   S pour sauter" -ForegroundColor White

    Write-Journal -Text "action $n - AFFICHEE : $($a.Do)" | Out-Null

    $skip = $false
    while ($true) {
        $key = [Console]::ReadKey($true)
        if ($key.Key -eq [ConsoleKey]::Spacebar) { break }
        if ($key.Key -eq [ConsoleKey]::S) { $skip = $true; break }
    }

    if ($skip) {
        Write-Host "  -> SAUTEE" -ForegroundColor DarkYellow
        Write-Journal -Text "action $n - SAUTEE" | Out-Null
        continue
    }

    $line = Write-Journal -Text "action $n - FAITE    : $($a.Do)"
    Write-Host "  -> $line" -ForegroundColor Green

    if ($proc.HasExited) { throw "USBPcapCMD s'est arrete en cours de route !" }

    Write-Host "  silence de $GuardSeconds s..." -NoNewline -ForegroundColor DarkGray
    Write-Journal -Text "action $n - debut silence de garde" | Out-Null
    Start-Sleep -Seconds $GuardSeconds
    Write-Host " ok"
}

# --- fin ------------------------------------------------------------------

Write-Host ""
Write-Host "Silence final de 10 s..." -ForegroundColor Yellow
Write-Journal -Text "debut silence final" | Out-Null
Start-Sleep -Seconds 10
Write-Journal -Text "fin capture" | Out-Null

Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 3

Write-Host ""
Write-Host ("=" * 72) -ForegroundColor Cyan
if (Test-Path $pcap) {
    $size = (Get-Item $pcap).Length
    Write-Host ("pcap    : {0}  ({1:N0} octets / {2:N2} Mo)" -f $pcap, $size, ($size / 1MB)) -ForegroundColor Green
    if ($size -lt 10KB) { Write-Host "ATTENTION : fichier tres petit." -ForegroundColor Red }
} else {
    Write-Host "ERREUR : aucun pcap produit." -ForegroundColor Red
}
Write-Host "journal : $journal" -ForegroundColor Green
