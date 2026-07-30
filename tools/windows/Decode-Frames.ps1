<#
    Decodeur des trames NZXT extraites par tshark.

    Prend les .tsv produits par tshark (frame.number, frame.time_relative,
    usbhid.data) et met en forme les trames 0x2a 0x04 champ par champ.

    Sans accents : UTF-8 sans BOM + PowerShell 5.1.
#>
param(
    [Parameter(Mandatory = $true)][string] $Tsv,
    [switch] $ShowAllZero
)

function Hex2Bytes {
    param([string] $h)
    $h = $h -replace '[^0-9a-fA-F]', ''
    $b = New-Object byte[] ($h.Length / 2)
    for ($i = 0; $i -lt $b.Length; $i++) {
        $b[$i] = [Convert]::ToByte($h.Substring($i * 2, 2), 16)
    }
    return $b
}

function GrbToHtml {
    param([byte[]] $b, [int] $off)
    # ordre confirme par texte clair connu : G, R, B
    $g = $b[$off]; $r = $b[$off + 1]; $bl = $b[$off + 2]
    return ('#{0:x2}{1:x2}{2:x2}' -f $r, $g, $bl)
}

Get-Content $Tsv | Where-Object { $_.Trim() } | ForEach-Object {
    $p = $_ -split "`t"
    $num = $p[0]; $t = [double]$p[1]; $hex = $p[2]
    if (-not $hex) { return }
    $b = Hex2Bytes $hex

    $allZero = -not ($b | Where-Object { $_ -ne 0 })
    if ($allZero -and -not $ShowAllZero) { return }

    if ($b.Length -ge 8 -and $b[0] -eq 0x2a -and $b[1] -eq 0x04) {
        # trame de configuration LED
        $mask = $b[2]
        $chans = @()
        for ($i = 0; $i -lt 8; $i++) { if ($mask -band (1 -shl $i)) { $chans += ($i + 1) } }

        # Le trailer occupe les 4 derniers octets, quelle que soit la longueur
        # reelle de la charge utile (60 octets observes, pas 64).
        $trailerStart = $b.Length - 4

        # combien de triplets non nuls a partir de l'offset 7 ?
        $triplets = @()
        for ($o = 7; $o + 2 -lt $trailerStart; $o += 3) {
            $triplets += (GrbToHtml $b $o)
        }
        # on ne garde que jusqu'au dernier triplet non nul
        $lastNonZero = -1
        for ($i = 0; $i -lt $triplets.Count; $i++) {
            if ($triplets[$i] -ne '#000000') { $lastNonZero = $i }
        }
        $shown = if ($lastNonZero -ge 0) { $triplets[0..$lastNonZero] } else { @('#000000') }

        "frame {0,6}  t={1,7:N2}  CONFIG LED  ({2} octets)" -f $num, $t, $b.Length
        "    [0..1] cmd        = {0:x2} {1:x2}" -f $b[0], $b[1]
        "    [2]    masque ch  = 0x{0:x2}  -> canaux {1}" -f $mask, ($chans -join ',')
        "    [3]    masque ch2 = 0x{0:x2}" -f $b[3]
        "    [4]    mode       = 0x{0:x2}" -f $b[4]
        "    [5]    ?          = 0x{0:x2}  ({0})" -f $b[5]
        "    [6]    ?          = 0x{0:x2}  ({0})" -f $b[6]
        "    [7..]  couleurs   = {0}   ({1} triplet(s) non nul(s))" -f ($shown -join ' '), ($lastNonZero + 1)
        "    [{0}..{1}] trailer = {2:x2} {3:x2} {4:x2} {5:x2}" -f $trailerStart, ($b.Length - 1),
            $b[$trailerStart], $b[$trailerStart + 1], $b[$trailerStart + 2], $b[$trailerStart + 3]
        ""
    }
    else {
        $trim = ($hex -replace '(00)+$', '')
        if (-not $trim) { $trim = '(zeros)' }
        "frame {0,6}  t={1,7:N2}  {2}" -f $num, $t, $trim
    }
}
