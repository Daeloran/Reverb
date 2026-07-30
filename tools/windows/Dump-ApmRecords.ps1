<#
    Decode les enregistrements d'appel d'une capture API Monitor.

    process/N/calls est un index d'offsets sur 8 octets, pointant dans
    process/N/data. On extrait les premiers enregistrements pour reperer ou
    se trouvent le code IOCTL et le tampon d'entree.

    Sans accents (UTF-8 sans BOM + PowerShell 5.1).
#>
param(
    [Parameter(Mandatory = $true)][string] $Apmx,
    [int] $Premiers = 6,
    [int] $TailleData = 60000
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem

function Read-Entry {
    param($Zip, [string] $Pattern, [int] $Max)
    $e = $Zip.Entries | Where-Object { $_.FullName -like $Pattern } |
         Sort-Object Length -Descending | Select-Object -First 1
    if (-not $e) { throw "entree absente : $Pattern" }
    $s = $e.Open()
    $n = [Math]::Min($Max, $e.Length)
    $b = New-Object byte[] $n
    $got = 0
    while ($got -lt $n) { $r = $s.Read($b, $got, $n - $got); if ($r -le 0) { break }; $got += $r }
    $s.Dispose()
    return ,$b
}

$zip = [System.IO.Compression.ZipFile]::OpenRead($Apmx)
try {
    $calls = Read-Entry $zip 'process/*/calls' (8 * ($Premiers + 2))
    $data  = Read-Entry $zip 'process/*/data'  $TailleData
} finally { $zip.Dispose() }

$offsets = @()
for ($i = 0; $i + 7 -lt $calls.Length; $i += 8) {
    $offsets += [BitConverter]::ToInt64($calls, $i)
}
"offsets des premiers enregistrements : " + (($offsets | Select-Object -First ($Premiers+1)) -join ', ')
""

# codes IOCTL vus a l ecran. On compare en UInt32 : certaines valeurs de
# l enregistrement depassent Int32 et un transtypage echouerait.
[uint32] $IOCTL_LECTURE  = 2249560   # 0x225358
[uint32] $IOCTL_ECRITURE = 2265940   # 0x229354
function Nom-Ioctl { param([uint32] $v)
    if ($v -eq $IOCTL_LECTURE)  { return 'lecture 0x225358' }
    if ($v -eq $IOCTL_ECRITURE) { return 'ecriture 0x229354' }
    return $null
}

for ($k = 0; $k -lt $Premiers -and $k + 1 -lt $offsets.Count; $k++) {
    $start = [int]$offsets[$k]
    $end   = [int]$offsets[$k+1]
    if ($end -gt $data.Length) { break }
    $len = $end - $start
    "===== enregistrement $k  offset=$start  longueur=$len ====="

    # reperer les codes IOCTL en little-endian dans l enregistrement
    for ($i = $start; $i -le $end - 4; $i++) {
        $v = [BitConverter]::ToUInt32($data, $i)
        $nom = Nom-Ioctl $v
        if ($nom) {
            "  IOCTL {0} ({1}) trouve a l offset relatif {2}" -f $v, $nom, ($i - $start)
            # la taille du tampon (10) et le tampon lui-meme sont a proximite
            $from = [Math]::Max($start, $i - 16)
            $to   = [Math]::Min($end - 1, $i + 80)
            for ($j = $from; $j -le $to; $j += 16) {
                $e2 = [Math]::Min($j + 15, $to)
                $h = ($data[$j..$e2] | ForEach-Object { '{0:x2}' -f $_ }) -join ' '
                $a = -join ($data[$j..$e2] | ForEach-Object { if ($_ -ge 32 -and $_ -lt 127) { [char]$_ } else { '.' } })
                "    {0,5}: {1,-48} {2}" -f ($j - $start), $h, $a
            }
            break
        }
    }
    ""
}
