<#
    Analyse d'une capture cible 1 : extrait les trames OUT des controleurs RGB,
    les decode, et les rattache a l'action du journal qui les a provoquees.

    Sans accents (UTF-8 sans BOM + PowerShell 5.1).

    Usage :
        .\Analyse-Cible1.ps1 -Pcap .\cible1-....pcap -Journal .\cible1-....journal.txt
#>
param(
    [Parameter(Mandatory = $true)][string] $Pcap,
    [Parameter(Mandatory = $true)][string] $Journal,
    [switch] $ShowPolling
)

$ErrorActionPreference = 'Stop'
$tshark = "C:\Program Files\Wireshark\tshark.exe"

# --- actions du journal ---------------------------------------------------

$actions = @()
foreach ($l in Get-Content $Journal) {
    if ($l -match '^(\d\d):(\d\d):(\d\d)\.(\d\d\d)\s+action (\d+) - (AFFICHEE|FAITE)\s*:\s*(.*)$') {
        $actions += [pscustomobject]@{
            Time  = [datetime]::ParseExact("$($matches[1]):$($matches[2]):$($matches[3]).$($matches[4])",
                                           'HH:mm:ss.fff', $null)
            Num   = [int]$matches[5]
            Kind  = $matches[6]
            Label = $matches[7]
        }
    }
}

function Get-Action {
    param([datetime] $t)
    # l'action en cours est la derniere AFFICHEE avant l'instant t
    $a = $actions | Where-Object { $_.Kind -eq 'AFFICHEE' -and $_.Time -le $t } |
         Sort-Object Time | Select-Object -Last 1
    if ($a) { return $a }
    return $null
}

# --- extraction -----------------------------------------------------------

$raw = & $tshark -r $Pcap -Y "usb.endpoint_address==0x02 && usbhid.data" `
        -T fields -e frame.number -e frame.time_epoch -e usb.device_address -e usbhid.data `
        -E separator=`t 2>$null

function Hex2Bytes {
    param([string] $h)
    $h = $h -replace '[^0-9a-fA-F]', ''
    if ($h.Length -lt 2) { return @() }
    $b = New-Object byte[] ($h.Length / 2)
    for ($i = 0; $i -lt $b.Length; $i++) { $b[$i] = [Convert]::ToByte($h.Substring($i*2,2),16) }
    return $b
}

$lastAction = -1

foreach ($line in $raw) {
    if (-not $line.Trim()) { continue }
    $p = $line -split "`t"
    $num = $p[0]
    $t   = [DateTimeOffset]::FromUnixTimeMilliseconds([long]([double]$p[1] * 1000)).LocalDateTime
    $dev = $p[2]
    $b   = Hex2Bytes $p[3]
    if ($b.Count -lt 2) { continue }

    # trame entierement nulle : bourrage
    if (-not ($b | Where-Object { $_ -ne 0 })) { continue }

    # polling vitesse ventilateur, tres repetitif
    $isPolling = ($b[0] -eq 0x62 -and $b[1] -eq 0x01)
    if ($isPolling -and -not $ShowPolling) { continue }

    $act = Get-Action $t
    $actNum = if ($act) { $act.Num } else { 0 }
    if ($actNum -ne $lastAction) {
        ""
        "=============================================================================="
        if ($act) { "ACTION $($act.Num) : $($act.Label)" } else { "(avant la premiere action)" }
        "=============================================================================="
        $lastAction = $actNum
    }

    $ts = $t.ToString('HH:mm:ss.fff')

    if ($b.Count -ge 8 -and $b[0] -eq 0x2a -and $b[1] -eq 0x04) {
        $trailerStart = 56
        $triplets = @()
        for ($o = 7; $o + 2 -lt $trailerStart; $o += 3) {
            $g = $b[$o]; $r = $b[$o+1]; $bl = $b[$o+2]
            $triplets += ('#{0:x2}{1:x2}{2:x2}' -f $r, $g, $bl)
        }
        $last = -1
        for ($i = 0; $i -lt $triplets.Count; $i++) { if ($triplets[$i] -ne '#000000') { $last = $i } }
        $shown = if ($last -ge 0) { $triplets[0..$last] } else { @() }

        "[{0}] dev{1} f{2,-6} 2a04  mask=0x{3:x2} m2=0x{4:x2}  mode=0x{5:x2}  b5=0x{6:x2}({6})  b6=0x{7:x2}({7})  trailer={8:x2} {9:x2} {10:x2} {11:x2}" -f `
            $ts, $dev, $num, $b[2], $b[3], $b[4], $b[5], $b[6], $b[56], $b[57], $b[58], $b[59]
        if ($shown.Count -gt 0) {
            "         couleurs ({0}) : {1}" -f $shown.Count, ($shown -join ' ')
        } else {
            "         couleurs (0) : aucune (tout a zero)"
        }
    }
    else {
        $hex = ($b | ForEach-Object { '{0:x2}' -f $_ }) -join ''
        $hex = $hex -replace '(00)+$', ''
        if (-not $hex) { $hex = '(zeros)' }
        "[{0}] dev{1} f{2,-6} {3}" -f $ts, $dev, $num, $hex
    }
}
