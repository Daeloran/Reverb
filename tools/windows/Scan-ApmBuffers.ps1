<#
    Cherche des tableaux de LED dans les tampons captures par API Monitor.

    Le format .apmx64 est une archive ZIP (precedee d'un en-tete texte et de la
    marque RBAPM). L'entree process/N/data contient les tampons bruts.

    Plutot que de decoder un format proprietaire, on exploite le texte clair
    connu : quatre barrettes reglees sur quatre couleurs franches. Un tableau
    de LED se repere a une suite de triplets identiques non nuls.

    Sans accents (UTF-8 sans BOM + PowerShell 5.1).
#>
param(
    [Parameter(Mandatory = $true)][string] $Apmx,
    [int] $MinRepeats = 4,
    [int] $MaxFindings = 400
)

$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.IO.Compression.FileSystem
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.IO;

public class ApmScan
{
    public class Hit
    {
        public long Offset;
        public byte R, G, B;
        public int Repeats;
        public string Context;
    }

    // Parcourt le flux et signale toute suite d'au moins minRepeats triplets
    // identiques et non nuls.
    public static List<Hit> Scan(Stream s, int minRepeats, int maxFindings)
    {
        var hits = new List<Hit>();
        const int CHUNK = 4 << 20;
        const int OVERLAP = 256;
        byte[] buf = new byte[CHUNK + OVERLAP];
        int carried = 0;
        long baseOffset = 0;

        while (true)
        {
            int read = s.Read(buf, carried, CHUNK);
            if (read <= 0) break;
            int avail = carried + read;

            int i = 0;
            while (i + 3 * minRepeats <= avail)
            {
                byte r = buf[i], g = buf[i + 1], b = buf[i + 2];
                if (r == 0 && g == 0 && b == 0) { i++; continue; }

                int rep = 1;
                int j = i + 3;
                while (j + 2 < avail && buf[j] == r && buf[j + 1] == g && buf[j + 2] == b)
                { rep++; j += 3; }

                if (rep >= minRepeats)
                {
                    var h = new Hit();
                    h.Offset = baseOffset + i;
                    h.R = r; h.G = g; h.B = b; h.Repeats = rep;

                    int cs = Math.Max(0, i - 24);
                    int ce = Math.Min(avail, j + 24);
                    var sb = new System.Text.StringBuilder();
                    for (int k = cs; k < ce; k++)
                    {
                        if (k == i) sb.Append("[ ");
                        if (k == j) sb.Append("] ");
                        sb.Append(buf[k].ToString("x2")).Append(' ');
                    }
                    h.Context = sb.ToString();
                    hits.Add(h);
                    if (hits.Count >= maxFindings) return hits;
                    i = j;
                }
                else i++;
            }

            carried = avail - i;
            if (carried > OVERLAP) carried = OVERLAP;
            Array.Copy(buf, avail - carried, buf, 0, carried);
            baseOffset += avail - carried;
        }
        return hits;
    }
}
'@

$zip = [System.IO.Compression.ZipFile]::OpenRead($Apmx)
try {
    $entry = $zip.Entries | Where-Object { $_.FullName -like 'process/*/data' } |
             Sort-Object Length -Descending | Select-Object -First 1
    if (-not $entry) { throw "Aucune entree process/N/data dans l archive." }
    "entree : $($entry.FullName)  ($('{0:N0}' -f $entry.Length) octets decompresses)"

    $st = $entry.Open()
    try {
        $hits = [ApmScan]::Scan($st, $MinRepeats, $MaxFindings)
    } finally { $st.Dispose() }
} finally { $zip.Dispose() }

"trouvailles : $($hits.Count)"
""
"===== couleurs rencontrees (triplet brut, tel qu il apparait) ====="
$hits | Group-Object { '{0:x2} {1:x2} {2:x2}' -f $_.R, $_.G, $_.B } |
    Sort-Object Count -Descending | Select-Object -First 20 |
    ForEach-Object {
        $ex = $_.Group[0]
        "{0,6}x  {1}   repetitions max={2}" -f $_.Count, $_.Name, (($_.Group | Measure-Object Repeats -Maximum).Maximum)
    }
""
"===== echantillons avec contexte ====="
$hits | Group-Object { '{0:x2} {1:x2} {2:x2}' -f $_.R, $_.G, $_.B } |
    Sort-Object Count -Descending | Select-Object -First 8 |
    ForEach-Object {
        $h = $_.Group[0]
        ""
        "--- {0}  x{1} repetitions  offset {2} ---" -f $_.Name, $h.Repeats, $h.Offset
        $h.Context
    }
