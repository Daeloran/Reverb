<#
    Extrait les tampons d'entree des appels DeviceIoControl captures par
    API Monitor sur CorsairDeviceControlService, et en fait la statistique.

    Ancrage : le code IOCTL apparait a un offset fixe dans l'enregistrement ;
    le tampon de 10 octets suit 40 octets plus loin.

    Sans accents (UTF-8 sans BOM + PowerShell 5.1).
#>
param(
    [Parameter(Mandatory = $true)][string] $Apmx,
    [int] $Decalage = 40,
    [int] $Taille = 10,
    [int] $MaxRecords = 700000
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.IO;

public class ApmSmbus
{
    public class Row { public uint Ioctl; public string Buf; }

    public static List<Row> Parse(Stream calls, Stream data, int decalage, int taille, int maxRec)
    {
        // index des offsets
        var idx = new List<long>();
        byte[] c8 = new byte[8];
        while (idx.Count < maxRec + 1)
        {
            int got = 0;
            while (got < 8) { int r = calls.Read(c8, got, 8 - got); if (r <= 0) break; got += r; }
            if (got < 8) break;
            idx.Add(BitConverter.ToInt64(c8, 0));
        }

        var rows = new List<Row>();
        long pos = 0;
        byte[] rec = new byte[65536];

        for (int k = 0; k + 1 < idx.Count; k++)
        {
            int len = (int)(idx[k + 1] - idx[k]);
            if (len <= 0 || len > rec.Length) break;
            int got = 0;
            while (got < len) { int r = data.Read(rec, got, len - got); if (r <= 0) break; got += r; }
            if (got < len) break;
            pos += len;

            // recherche du code IOCTL : device type 0x22, method buffered
            for (int i = 0; i + 4 <= len - decalage - taille; i++)
            {
                uint v = BitConverter.ToUInt32(rec, i);
                if ((v >> 16) == 0x22 && (v & 3) == 0 && v > 0x220000 && v < 0x230000)
                {
                    int b = i + decalage;
                    var sb = new System.Text.StringBuilder();
                    for (int j = 0; j < taille; j++) sb.Append(rec[b + j].ToString("x2")).Append(' ');
                    var row = new Row(); row.Ioctl = v; row.Buf = sb.ToString().Trim();
                    rows.Add(row);
                    break;
                }
            }
        }
        return rows;
    }
}
'@

$zip = [System.IO.Compression.ZipFile]::OpenRead($Apmx)
try {
    $eC = $zip.Entries | Where-Object { $_.FullName -like 'process/*/calls' } | Select-Object -First 1
    $eD = $zip.Entries | Where-Object { $_.FullName -like 'process/*/data' }  | Select-Object -First 1
    $sC = $eC.Open(); $sD = $eD.Open()
    try { $rows = [ApmSmbus]::Parse($sC, $sD, $Decalage, $Taille, $MaxRecords) }
    finally { $sC.Dispose(); $sD.Dispose() }
} finally { $zip.Dispose() }

"appels analyses : $($rows.Count)"
""
"===== repartition par code IOCTL ====="
$rows | Group-Object Ioctl | Sort-Object Count -Descending |
  ForEach-Object { "{0,9}x  0x{1:x6}" -f $_.Count, [uint32]$_.Name }
""
"===== tampons les plus frequents ====="
$rows | Group-Object Buf | Sort-Object Count -Descending | Select-Object -First 30 |
  ForEach-Object { "{0,9}x  {1}" -f $_.Count, $_.Name }
""
"===== variabilite octet par octet ====="
for ($p = 0; $p -lt $Taille; $p++) {
    $vals = $rows | ForEach-Object { ($_.Buf -split ' ')[$p] } | Group-Object |
            Sort-Object Count -Descending
    $apercu = ($vals | Select-Object -First 6 | ForEach-Object { "{0}({1})" -f $_.Name, $_.Count }) -join ' '
    "octet {0} : {1,4} valeurs distinctes   {2}" -f $p, $vals.Count, $apercu
}
