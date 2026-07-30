<#
    Recense les chaines ASCII presentes dans l'entree process/N/data d'une
    capture API Monitor, pour decouvrir quelles fonctions ont ete tracees.

    Sans accents (UTF-8 sans BOM + PowerShell 5.1).
#>
param(
    [Parameter(Mandatory = $true)][string] $Apmx,
    [int] $MinLen = 5,
    [string] $Filtre = ''
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.IO;
using System.Text;

public class ApmStrings
{
    public static Dictionary<string,int> Scan(Stream s, int minLen)
    {
        var counts = new Dictionary<string,int>();
        const int CHUNK = 4 << 20;
        byte[] buf = new byte[CHUNK];
        var cur = new StringBuilder();

        while (true)
        {
            int read = s.Read(buf, 0, CHUNK);
            if (read <= 0) break;
            for (int i = 0; i < read; i++)
            {
                byte b = buf[i];
                bool printable = (b >= 0x20 && b < 0x7F);
                if (printable) { cur.Append((char)b); }
                else
                {
                    if (cur.Length >= minLen)
                    {
                        string v = cur.ToString();
                        int c; counts.TryGetValue(v, out c); counts[v] = c + 1;
                    }
                    cur.Length = 0;
                }
            }
        }
        if (cur.Length >= minLen)
        {
            string v = cur.ToString();
            int c; counts.TryGetValue(v, out c); counts[v] = c + 1;
        }
        return counts;
    }
}
'@

$zip = [System.IO.Compression.ZipFile]::OpenRead($Apmx)
try {
    $entry = $zip.Entries | Where-Object { $_.FullName -like 'process/*/data' } |
             Sort-Object Length -Descending | Select-Object -First 1
    "entree : $($entry.FullName)  ($('{0:N0}' -f $entry.Length) octets)"
    $st = $entry.Open()
    try { $counts = [ApmStrings]::Scan($st, $MinLen) } finally { $st.Dispose() }
} finally { $zip.Dispose() }

"chaines distinctes : $($counts.Count)"
""
$res = $counts.GetEnumerator()
if ($Filtre) { $res = $res | Where-Object { $_.Key -match $Filtre } }
"===== chaines les plus frequentes ====="
$res | Sort-Object Value -Descending | Select-Object -First 60 |
    ForEach-Object { "{0,9}x  {1}" -f $_.Value, $_.Key }
