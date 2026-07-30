<#
    Extrait une trame d'image du Kraken depuis une capture et la rend en PNG.

    Hypothese a valider : 640x640, RGB888 brut, 1 228 800 octets.
    Les captures USBPcap sont tronquees a 65 535 octets, on ne reconstruit
    donc que les premieres lignes.

    Sans accents (UTF-8 sans BOM + PowerShell 5.1).
#>
param(
    [Parameter(Mandatory = $true)][string] $Pcap,
    [int] $Frame = 0,
    [int] $Width = 640,
    [string] $Out = "$PSScriptRoot\kraken-frame.png",
    [ValidateSet('RGB','BGR')][string] $Ordre = 'RGB'
)

$ErrorActionPreference = 'Stop'
$tshark = "C:\Program Files\Wireshark\tshark.exe"

if ($Frame -le 0) {
    $Frame = [int](& $tshark -r $Pcap -Y "usb.endpoint_address==0x02 && usb.data_len==1228800" `
                    -T fields -e frame.number 2>$null | Select-Object -First 1)
}
"trame retenue : $Frame"

$hex = (& $tshark -r $Pcap -Y "frame.number==$Frame" -T fields -e usb.capdata 2>$null | Out-String)
$hex = $hex -replace '[^0-9a-fA-F]', ''
$nbytes = [int]($hex.Length / 2)
"octets disponibles : $nbytes sur 1228800 attendus"

$bytes = New-Object byte[] $nbytes
for ($i = 0; $i -lt $nbytes; $i++) {
    $bytes[$i] = [Convert]::ToByte($hex.Substring($i*2, 2), 16)
}

# statistiques : une image reelle n'est ni uniforme ni aleatoire
$distinct = ($bytes | Select-Object -Unique).Count
"valeurs d octets distinctes : $distinct sur 256"
$zeros = ($bytes | Where-Object { $_ -eq 0 }).Count
"octets nuls : {0} ({1:N1} %)" -f $zeros, (100 * $zeros / $nbytes)

$rows = [int][Math]::Floor($nbytes / ($Width * 3))
"lignes completes reconstituables : $rows"
if ($rows -lt 1) { throw "Pas assez de donnees pour une seule ligne." }

Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Width, $rows)
for ($y = 0; $y -lt $rows; $y++) {
    for ($x = 0; $x -lt $Width; $x++) {
        $o = ($y * $Width + $x) * 3
        if ($Ordre -eq 'RGB') { $r = $bytes[$o]; $g = $bytes[$o+1]; $b = $bytes[$o+2] }
        else                  { $b = $bytes[$o]; $g = $bytes[$o+1]; $r = $bytes[$o+2] }
        $bmp.SetPixel($x, $y, [System.Drawing.Color]::FromArgb($r, $g, $b))
    }
}
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
"image ecrite : $Out  ($Width x $rows)"
