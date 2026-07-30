<#
    Reconstitue les transferts SMBus par bloc vers les controleurs RGB de la
    RAM Corsair, a partir des acces aux ports captures par API Monitor.

    Sequence d'un bloc :
        W base+4 = adresse 7 bits decalee
        W base+3 = registre
        W base+5 = nombre d octets
        W base+7 = donnees, un octet par acces
        W base+2 = 0x54  (START | protocole bloc)

    Sans accents (UTF-8 sans BOM + PowerShell 5.1).
#>
param(
    [Parameter(Mandatory = $true)][string] $Apmx,
    [int] $Decalage = 40,
    [int] $MaxAffiches = 40,
    [string] $Csv = ''
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.IO;

public class BlocRebuild
{
    public class Bloc
    {
        public int Addr7; public bool Read; public uint Reg; public uint Count; public byte[] Data;
    }

    public static List<Bloc> Parse(Stream calls, Stream data, int decalage)
    {
        var idx = new List<long>();
        byte[] c8 = new byte[8];
        while (true)
        {
            int got = 0;
            while (got < 8) { int r = calls.Read(c8, got, 8 - got); if (r <= 0) break; got += r; }
            if (got < 8) break;
            idx.Add(BitConverter.ToInt64(c8, 0));
        }

        var blocs = new List<Bloc>();
        byte[] rec = new byte[65536];
        uint addr = 0, reg = 0, count = 0;
        var payload = new List<byte>();

        for (int k = 0; k + 1 < idx.Count; k++)
        {
            int len = (int)(idx[k + 1] - idx[k]);
            if (len <= 0 || len > rec.Length) break;
            int got = 0;
            while (got < len) { int r = data.Read(rec, got, len - got); if (r <= 0) break; got += r; }
            if (got < len) break;

            // Ancrage strict sur les deux seuls codes IOCTL reels : une
            // recherche laxiste produisait de faux positifs qui corrompaient
            // les blocs.
            for (int i = 0; i + 4 <= len - decalage - 10; i++)
            {
                uint v = BitConverter.ToUInt32(rec, i);
                if (v != 0x225358 && v != 0x229354) continue;

                bool write = (v == 0x229354);
                int b = i + decalage;
                uint port = BitConverter.ToUInt32(rec, b);
                uint val  = BitConverter.ToUInt32(rec, b + 4);

                if (write)
                {
                    switch (port)
                    {
                        case 0x0B04: addr = val; break;
                        case 0x0B03: reg = val; break;
                        case 0x0B05: count = val; break;
                        case 0x0B07: payload.Add((byte)val); break;
                        case 0x0B02:
                            if ((val & 0x40) != 0)
                            {
                                int proto = (int)((val >> 2) & 7);
                                if (proto == 5 && payload.Count > 0)
                                {
                                    var bl = new Bloc();
                                    bl.Addr7 = (int)(addr >> 1);
                                    bl.Read  = (addr & 1) != 0;
                                    bl.Reg = reg; bl.Count = count;
                                    bl.Data = payload.ToArray();
                                    blocs.Add(bl);
                                }
                                payload.Clear();
                            }
                            break;
                    }
                }
                break;
            }
        }
        return blocs;
    }
}
'@

$zip = [System.IO.Compression.ZipFile]::OpenRead($Apmx)
try {
    $sC = ($zip.Entries | Where-Object { $_.FullName -like 'process/*/calls' } | Select-Object -First 1).Open()
    $sD = ($zip.Entries | Where-Object { $_.FullName -like 'process/*/data' }  | Select-Object -First 1).Open()
    try { $blocs = [BlocRebuild]::Parse($sC, $sD, $Decalage) } finally { $sC.Dispose(); $sD.Dispose() }
} finally { $zip.Dispose() }

"blocs reconstitues : $($blocs.Count)"
""
"===== blocs par adresse et registre ====="
$blocs | Group-Object { '0x{0:x2}' -f $_.Addr7 }, { '0x{0:x2}' -f $_.Reg }, { $_.Data.Length } |
  Sort-Object Name | ForEach-Object { "{0,6}x  addr/reg/taille {1}" -f $_.Count, $_.Name }
""

# Les blocs de 32 octets (registre 0x31) portent le compteur de LED puis les
# triplets ; le bloc de 3 octets (registre 0x32) complete le dernier.
"===== couleurs extraites des blocs du registre 0x31 ====="
"(octet 0 = nombre de LED, puis triplets)"
""
$vus = @{}
$n = 0
foreach ($b in $blocs) {
    if ($b.Reg -ne 0x31 -or $b.Data.Length -lt 4) { continue }
    $nbLed = $b.Data[0]
    $t = @()
    for ($i = 1; $i + 2 -lt $b.Data.Length; $i += 3) {
        $t += ('{0:x2}{1:x2}{2:x2}' -f $b.Data[$i], $b.Data[$i+1], $b.Data[$i+2])
    }
    $distinct = ($t | Select-Object -Unique)
    $cle = ('0x{0:x2}' -f $b.Addr7) + '|' + ($distinct -join ',')
    if (-not $vus.ContainsKey($cle)) {
        $vus[$cle] = 1
        $n++
        if ($n -le $MaxAffiches) {
            "addr 0x{0:x2}  nbLed={1}  triplets distincts : {2}" -f $b.Addr7, $nbLed, ($distinct -join ' ')
        }
    } else { $vus[$cle]++ }
}
""
"combinaisons distinctes rencontrees : $($vus.Count)"

if ($Csv) {
    $rows = foreach ($b in $blocs) {
        [pscustomobject]@{
            Addr  = '0x{0:x2}' -f $b.Addr7
            Reg   = '0x{0:x2}' -f $b.Reg
            Count = $b.Count
            Len   = $b.Data.Length
            Data  = ($b.Data | ForEach-Object { '{0:x2}' -f $_ }) -join ' '
        }
    }
    $rows | Export-Csv -Path $Csv -NoTypeInformation -Encoding UTF8
    "csv ecrit : $Csv"
}
